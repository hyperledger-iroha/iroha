package org.hyperledger.iroha.sdk.client

import java.io.ByteArrayOutputStream
import java.math.BigInteger
import java.nio.charset.StandardCharsets
import java.security.MessageDigest
import java.security.SecureRandom
import org.bouncycastle.crypto.engines.ChaChaEngine
import org.bouncycastle.crypto.params.KeyParameter
import org.bouncycastle.crypto.params.ParametersWithIV
import org.hyperledger.iroha.sdk.crypto.IrohaHash
import org.hyperledger.iroha.sdk.norito.SchemaHash

/** Builds framed Norito BFV identifier ciphertext envelopes from client-side input. */
internal object IdentifierBfvEnvelopeBuilder {
    private const val SCHEMA_NAME = "iroha_crypto::fhe_bfv::BfvIdentifierCiphertext"
    private val RUST_ENCRYPT_DOMAIN = "iroha.crypto.fhe.bfv.encrypt.v1".toByteArray(StandardCharsets.UTF_8)
    private val RUST_SLOT_DOMAIN = "iroha.crypto.fhe.bfv.identifier.slot.v1".toByteArray(StandardCharsets.UTF_8)
    private val PRG_DOMAIN = "iroha.sdk.identifier.bfv.prg.v1".toByteArray(StandardCharsets.UTF_8)
    private val SLOT_DOMAIN = "iroha.sdk.identifier.bfv.slot.v1".toByteArray(StandardCharsets.UTF_8)
    private val U_DOMAIN = "iroha.sdk.identifier.bfv.u.v1".toByteArray(StandardCharsets.UTF_8)
    private val E1_DOMAIN = "iroha.sdk.identifier.bfv.e1.v1".toByteArray(StandardCharsets.UTF_8)
    private val E2_DOMAIN = "iroha.sdk.identifier.bfv.e2.v1".toByteArray(StandardCharsets.UTF_8)
    private val NORITO_MAGIC = byteArrayOf('N'.code.toByte(), 'R'.code.toByte(), 'T'.code.toByte(), '0'.code.toByte())
    private const val NORITO_COMPACT_LEN_FLAG = 0x02
    private const val BFV_IDENTIFIER_MAX_INPUT_BYTES = 63
    private const val CRC64_POLY = -0x3693a86a2878f0beL // 0xC96C5795D7870F42L
    private val CRC64_TABLE = buildCrc64Table()
    private val SECURE_RANDOM = SecureRandom()

    @JvmStatic
    fun encrypt(policy: IdentifierPolicySummary, input: String, seedOverride: ByteArray?): String {
        require("bfv-v1".equals(policy.inputEncryption, ignoreCase = true)) {
            "Policy ${policy.policyId} does not publish BFV encrypted-input support"
        }
        val publicParameters = policy.inputEncryptionPublicParametersDecoded
            ?: throw IllegalArgumentException("Policy ${policy.policyId} is missing decoded BFV public parameters")
        val normalizedInput = policy.normalization.normalize(input, "input")
        val params = validate(publicParameters)
        val inputBytes = normalizedInput.toByteArray(StandardCharsets.UTF_8)
        require(inputBytes.size <= params.maxInputBytes) {
            "input exceeds maxInputBytes ${params.maxInputBytes}"
        }
        val seed = seedOverride?.copyOf() ?: randomSeed()
        val scalars = encodeIdentifierScalars(params.maxInputBytes, inputBytes)
        val slots = scalars.mapIndexed { index, scalar -> encryptScalar(params, scalar, seed, index) }
        return bytesToHex(frameNorito(encodeEnvelopePayload(slots, params.useCompactNoritoLengths), params.useCompactNoritoLengths))
    }

    private fun randomSeed(): ByteArray {
        val seed = ByteArray(32)
        SECURE_RANDOM.nextBytes(seed)
        return seed
    }

    private fun validate(publicParameters: IdentifierBfvPublicParameters): ValidatedParameters {
        val params = publicParameters.parameters
        val polynomialDegree = Math.toIntExact(params.polynomialDegree)
        require(polynomialDegree >= 2 && (polynomialDegree and (polynomialDegree - 1)) == 0) {
            "BFV polynomialDegree must be a power of two and at least 2"
        }
        require(params.decompositionBaseLog in 1..16) {
            "BFV decompositionBaseLog must be within 1..=16"
        }
        val plaintextModulus = toUnsignedBigInteger(params.plaintextModulus)
        val ciphertextModulus = toUnsignedBigInteger(params.ciphertextModulus)
        require(plaintextModulus >= BigInteger.valueOf(2L)) { "BFV plaintextModulus must be at least 2" }
        require(ciphertextModulus > plaintextModulus) { "BFV ciphertextModulus must be greater than plaintextModulus" }
        require(ciphertextModulus.mod(plaintextModulus) == BigInteger.ZERO) { "BFV ciphertextModulus must be divisible by plaintextModulus" }
        val maxInputBytes = publicParameters.maxInputBytes
        require(maxInputBytes >= 1) { "BFV maxInputBytes must be at least 1" }
        require(BigInteger.valueOf(maxInputBytes.toLong()) < plaintextModulus) { "BFV maxInputBytes must fit into one plaintext slot" }
        require(maxInputBytes <= BFV_IDENTIFIER_MAX_INPUT_BYTES) {
            "BFV maxInputBytes must be at most $BFV_IDENTIFIER_MAX_INPUT_BYTES for the registered RAM-LFE BFV identifier profile"
        }
        val rawA = publicParameters.publicKey.a
        val rawB = publicParameters.publicKey.b
        require(rawA.size == polynomialDegree && rawB.size == polynomialDegree) {
            "BFV public-key polynomials must match polynomialDegree"
        }
        val a = Array(polynomialDegree) { toUnsignedBigInteger(rawA[it]) }
        val b = Array(polynomialDegree) { toUnsignedBigInteger(rawB[it]) }
        for (index in 0 until polynomialDegree) {
            require(a[index] < ciphertextModulus && b[index] < ciphertextModulus) {
                "BFV public-key coefficient exceeds ciphertextModulus"
            }
        }
        val useCompactNoritoLengths = when (publicParameters.noritoLengthEncoding?.trim()) {
            null, "", "u64-v1" -> false
            "compact-v1" -> true
            else -> throw IllegalArgumentException("BFV noritoLengthEncoding must be u64-v1 or compact-v1")
        }
        return ValidatedParameters(polynomialDegree, plaintextModulus, ciphertextModulus, maxInputBytes, useCompactNoritoLengths, a, b)
    }

    private fun encodeIdentifierScalars(maxInputBytes: Int, inputBytes: ByteArray): List<Long> {
        val scalars = ArrayList<Long>(maxInputBytes + 1)
        scalars.add(inputBytes.size.toLong())
        for (b in inputBytes) scalars.add((b.toInt() and 0xff).toLong())
        while (scalars.size < maxInputBytes + 1) scalars.add(0L)
        return scalars
    }

    private fun encryptScalar(params: ValidatedParameters, scalar: Long, seed: ByteArray, slotIndex: Int): CiphertextSlot {
        if (params.useCompactNoritoLengths) {
            val slotSeed = irohaHash(RUST_SLOT_DOMAIN, seed, littleEndianUInt64(slotIndex.toLong() and 0xffff_ffffL))
            return encryptScalarRust(params, scalar, slotSeed)
        }
        val slotSeed = sha512(SLOT_DOMAIN, seed, littleEndianUInt64((slotIndex.toLong() and 0xffff_ffffL)))
        val u = sampleSmallPolynomial(params, DeterministicStream(slotSeed, U_DOMAIN))
        val e1 = sampleErrorPolynomial(params, DeterministicStream(slotSeed, E1_DOMAIN))
        val e2 = sampleErrorPolynomial(params, DeterministicStream(slotSeed, E2_DOMAIN))
        val encoded = zeroPolynomial(params.polynomialDegree)
        encoded[0] = BigInteger.valueOf(scalar).mod(params.plaintextModulus)
        return CiphertextSlot(
            addPolynomialMod(addPolynomialMod(multiplyPolynomialMod(params, params.publicKeyB, u), e1, params.ciphertextModulus), encoded, params.ciphertextModulus),
            addPolynomialMod(multiplyPolynomialMod(params, params.publicKeyA, u), e2, params.ciphertextModulus)
        )
    }

    private fun encryptScalarRust(params: ValidatedParameters, scalar: Long, seed: ByteArray): CiphertextSlot {
        val rng = RustChaCha20Rng(irohaHash(RUST_ENCRYPT_DOMAIN, seed))
        val u = sampleSmallPolynomialRust(params, rng)
        val e1 = sampleErrorPolynomialRust(params, rng)
        val e2 = sampleErrorPolynomialRust(params, rng)
        val encoded = zeroPolynomial(params.polynomialDegree)
        encoded[0] = BigInteger.valueOf(scalar).mod(params.plaintextModulus)
        return CiphertextSlot(
            addPolynomialMod(addPolynomialMod(multiplyPolynomialMod(params, params.publicKeyB, u), e1, params.ciphertextModulus), encoded, params.ciphertextModulus),
            addPolynomialMod(multiplyPolynomialMod(params, params.publicKeyA, u), e2, params.ciphertextModulus)
        )
    }

    private fun sampleSmallPolynomial(params: ValidatedParameters, stream: DeterministicStream): Array<BigInteger> {
        return Array(params.polynomialDegree) { _ ->
            val sample = stream.nextByte().toInt() and 0xff
            when (sample % 3) {
                0 -> BigInteger.ZERO
                1 -> BigInteger.ONE
                else -> params.ciphertextModulus.subtract(BigInteger.ONE)
            }
        }
    }

    private fun sampleErrorPolynomial(params: ValidatedParameters, stream: DeterministicStream): Array<BigInteger> {
        return Array(params.polynomialDegree) { _ ->
            val sample = stream.nextByte().toInt() and 0xff
            when (sample % 3) {
                0 -> BigInteger.ZERO
                1 -> params.plaintextModulus
                else -> params.ciphertextModulus.subtract(params.plaintextModulus)
            }
        }
    }

    private fun sampleSmallPolynomialRust(params: ValidatedParameters, rng: RustChaCha20Rng): Array<BigInteger> {
        return Array(params.polynomialDegree) { _ ->
            when (rustRandomRange0To2(rng)) {
                0 -> BigInteger.ZERO
                1 -> BigInteger.ONE
                else -> params.ciphertextModulus.subtract(BigInteger.ONE)
            }
        }
    }

    private fun sampleErrorPolynomialRust(params: ValidatedParameters, rng: RustChaCha20Rng): Array<BigInteger> {
        return Array(params.polynomialDegree) { _ ->
            when (rustRandomRange0To2(rng)) {
                0 -> BigInteger.ZERO
                1 -> params.plaintextModulus
                else -> params.ciphertextModulus.subtract(params.plaintextModulus)
            }
        }
    }

    private fun rustRandomRange0To2(rng: RustChaCha20Rng): Int {
        val range = 3L
        val sample = rng.nextUInt32()
        val product = sample * range
        var result = (product ushr 32).toInt()
        val low = product and 0xffff_ffffL
        val biasedThreshold = (1L shl 32) - range
        if (low > biasedThreshold) {
            val retryHigh = (rng.nextUInt32() * range) ushr 32
            if (low + retryHigh > 0xffff_ffffL) result += 1
        }
        return result
    }

    private fun zeroPolynomial(degree: Int): Array<BigInteger> = Array(degree) { BigInteger.ZERO }

    private fun addPolynomialMod(lhs: Array<BigInteger>, rhs: Array<BigInteger>, modulus: BigInteger): Array<BigInteger> =
        Array(lhs.size) { lhs[it].add(rhs[it]).mod(modulus) }

    private fun multiplyPolynomialMod(params: ValidatedParameters, lhs: Array<BigInteger>, rhs: Array<BigInteger>): Array<BigInteger> {
        val output = zeroPolynomial(params.polynomialDegree)
        for (i in 0 until params.polynomialDegree) {
            for (j in 0 until params.polynomialDegree) {
                val term = lhs[i].multiply(rhs[j]).mod(params.ciphertextModulus)
                val target = i + j
                if (target < params.polynomialDegree) {
                    output[target] = output[target].add(term).mod(params.ciphertextModulus)
                } else {
                    output[target - params.polynomialDegree] = output[target - params.polynomialDegree].subtract(term).mod(params.ciphertextModulus)
                }
            }
        }
        return output
    }

    private fun encodeEnvelopePayload(slots: List<CiphertextSlot>, compact: Boolean): ByteArray {
        val payload = ByteArrayOutputStream()
        writeField(payload, encodeVecSlots(slots, compact), compact)
        return payload.toByteArray()
    }

    private fun encodeVecSlots(slots: List<CiphertextSlot>, compact: Boolean): ByteArray {
        val out = ByteArrayOutputStream()
        writeUInt64(out, slots.size.toLong())
        for (slot in slots) {
            val payload = encodeSlot(slot, compact)
            writeLength(out, payload.size.toLong(), compact)
            out.write(payload)
        }
        return out.toByteArray()
    }

    private fun encodeSlot(slot: CiphertextSlot, compact: Boolean): ByteArray {
        val out = ByteArrayOutputStream()
        writeField(out, encodeVecU64(slot.c0, compact), compact)
        writeField(out, encodeVecU64(slot.c1, compact), compact)
        return out.toByteArray()
    }

    private fun encodeVecU64(values: Array<BigInteger>, compact: Boolean): ByteArray {
        val out = ByteArrayOutputStream()
        writeUInt64(out, values.size.toLong())
        for (value in values) {
            val payload = littleEndianUInt64(toUnsignedLong(value))
            writeLength(out, payload.size.toLong(), compact)
            out.write(payload)
        }
        return out.toByteArray()
    }

    private fun frameNorito(payload: ByteArray, compact: Boolean): ByteArray {
        val out = ByteArrayOutputStream()
        out.write(NORITO_MAGIC)
        out.write(0)
        out.write(0)
        out.write(SchemaHash.hash16(SCHEMA_NAME))
        out.write(0)
        out.write(littleEndianUInt64(payload.size.toLong()))
        out.write(littleEndianUInt64(crc64(payload)))
        out.write(if (compact) NORITO_COMPACT_LEN_FLAG else 0)
        out.write(payload)
        return out.toByteArray()
    }

    private fun writeField(out: ByteArrayOutputStream, payload: ByteArray, compact: Boolean) {
        writeLength(out, payload.size.toLong(), compact)
        out.write(payload)
    }

    private fun writeLength(out: ByteArrayOutputStream, value: Long, compact: Boolean) {
        if (!compact) {
            writeUInt64(out, value)
            return
        }
        var remaining = value
        do {
            var byte = (remaining and 0x7fL).toInt()
            remaining = remaining ushr 7
            if (remaining != 0L) byte = byte or 0x80
            out.write(byte)
        } while (remaining != 0L)
    }

    private fun writeUInt64(out: ByteArrayOutputStream, value: Long) {
        out.write(littleEndianUInt64(value))
    }

    private fun littleEndianUInt64(value: Long): ByteArray {
        val out = ByteArray(8)
        for (index in 0 until 8) {
            out[index] = ((value ushr (index * 8)) and 0xff).toByte()
        }
        return out
    }

    private fun crc64(payload: ByteArray): Long {
        var crc = -1L
        for (value in payload) {
            val index = ((crc xor (value.toLong() and 0xffL)) and 0xffL).toInt()
            crc = CRC64_TABLE[index] xor (crc ushr 8)
        }
        return crc xor -1L
    }

    private fun buildCrc64Table(): LongArray {
        val table = LongArray(256)
        for (index in table.indices) {
            var crc = index.toLong()
            for (bit in 0 until 8) {
                crc = if ((crc and 1L) != 0L) (crc ushr 1) xor CRC64_POLY else crc ushr 1
            }
            table[index] = crc
        }
        return table
    }

    private fun toUnsignedBigInteger(value: Long): BigInteger {
        if (value >= 0) return BigInteger.valueOf(value)
        return BigInteger.valueOf(value and Long.MAX_VALUE).setBit(java.lang.Long.SIZE - 1)
    }

    private fun toUnsignedLong(value: BigInteger): Long = value.toLong()

    private fun sha512(vararg parts: ByteArray): ByteArray {
        val digest = MessageDigest.getInstance("SHA-512")
        for (part in parts) digest.update(part)
        return digest.digest()
    }

    private fun irohaHash(vararg parts: ByteArray): ByteArray {
        val out = ByteArrayOutputStream()
        for (part in parts) out.write(part)
        return IrohaHash.prehash(out.toByteArray())
    }

    private fun bytesToHex(bytes: ByteArray): String {
        val builder = StringBuilder(bytes.size * 2)
        for (value in bytes) {
            builder.append(Character.forDigit((value.toInt() ushr 4) and 0xf, 16))
            builder.append(Character.forDigit(value.toInt() and 0xf, 16))
        }
        return builder.toString()
    }

    private class DeterministicStream(seed: ByteArray, domain: ByteArray) {
        private val seed = seed.copyOf()
        private val domain = domain.copyOf()
        private var counter = 0L
        private var buffer = ByteArray(0)
        private var index = 0

        fun nextByte(): Byte {
            if (index >= buffer.size) refill()
            return buffer[index++]
        }

        private fun refill() {
            buffer = sha512(PRG_DOMAIN, domain, seed, littleEndianUInt64(counter))
            index = 0
            counter++
        }
    }

    private class RustChaCha20Rng(seed: ByteArray) {
        private val engine = ChaChaEngine(20)

        init {
            engine.init(true, ParametersWithIV(KeyParameter(seed.copyOf()), ByteArray(8)))
        }

        fun nextUInt32(): Long {
            val bytes = nextBytes(4)
            return ((bytes[0].toLong() and 0xffL)
                or ((bytes[1].toLong() and 0xffL) shl 8)
                or ((bytes[2].toLong() and 0xffL) shl 16)
                or ((bytes[3].toLong() and 0xffL) shl 24))
        }

        private fun nextBytes(length: Int): ByteArray {
            val input = ByteArray(length)
            val output = ByteArray(length)
            engine.processBytes(input, 0, input.size, output, 0)
            return output
        }
    }

    private class ValidatedParameters(
        val polynomialDegree: Int,
        val plaintextModulus: BigInteger,
        val ciphertextModulus: BigInteger,
        val maxInputBytes: Int,
        val useCompactNoritoLengths: Boolean,
        val publicKeyA: Array<BigInteger>,
        val publicKeyB: Array<BigInteger>
    )

    private class CiphertextSlot(val c0: Array<BigInteger>, val c1: Array<BigInteger>)
}
