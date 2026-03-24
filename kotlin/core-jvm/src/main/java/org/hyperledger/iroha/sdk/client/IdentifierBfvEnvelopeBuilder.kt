package org.hyperledger.iroha.sdk.client

import java.io.ByteArrayOutputStream
import java.math.BigInteger
import java.nio.charset.StandardCharsets
import java.security.MessageDigest
import java.security.SecureRandom

/** Builds framed Norito BFV identifier ciphertext envelopes from plaintext input. */
internal object IdentifierBfvEnvelopeBuilder {
    private const val SCHEMA_NAME = "iroha_crypto::fhe_bfv::BfvIdentifierCiphertext"
    private val PRG_DOMAIN = "iroha.sdk.identifier.bfv.prg.v1".toByteArray(StandardCharsets.UTF_8)
    private val SLOT_DOMAIN = "iroha.sdk.identifier.bfv.slot.v1".toByteArray(StandardCharsets.UTF_8)
    private val U_DOMAIN = "iroha.sdk.identifier.bfv.u.v1".toByteArray(StandardCharsets.UTF_8)
    private val E1_DOMAIN = "iroha.sdk.identifier.bfv.e1.v1".toByteArray(StandardCharsets.UTF_8)
    private val E2_DOMAIN = "iroha.sdk.identifier.bfv.e2.v1".toByteArray(StandardCharsets.UTF_8)
    private val NORITO_MAGIC = byteArrayOf('N'.code.toByte(), 'R'.code.toByte(), 'T'.code.toByte(), '0'.code.toByte())
    private const val FNV_OFFSET = -0x340d631b7cddd335L // 0xcbf29ce484222325L
    private const val FNV_PRIME = 0x100000001b3L
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
        return bytesToHex(frameNorito(encodeEnvelopePayload(slots)))
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
        require(plaintextModulus >= BigInteger.TWO) { "BFV plaintextModulus must be at least 2" }
        require(ciphertextModulus > plaintextModulus) { "BFV ciphertextModulus must be greater than plaintextModulus" }
        require(ciphertextModulus.mod(plaintextModulus) == BigInteger.ZERO) { "BFV ciphertextModulus must be divisible by plaintextModulus" }
        val maxInputBytes = publicParameters.maxInputBytes
        require(maxInputBytes >= 1) { "BFV maxInputBytes must be at least 1" }
        require(BigInteger.valueOf(maxInputBytes.toLong()) < plaintextModulus) { "BFV maxInputBytes must fit into one plaintext slot" }
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
        return ValidatedParameters(polynomialDegree, ciphertextModulus, ciphertextModulus.divide(plaintextModulus), maxInputBytes, a, b)
    }

    private fun encodeIdentifierScalars(maxInputBytes: Int, inputBytes: ByteArray): List<Long> {
        val scalars = ArrayList<Long>(maxInputBytes + 1)
        scalars.add(inputBytes.size.toLong())
        for (b in inputBytes) scalars.add((b.toInt() and 0xff).toLong())
        while (scalars.size < maxInputBytes + 1) scalars.add(0L)
        return scalars
    }

    private fun encryptScalar(params: ValidatedParameters, scalar: Long, seed: ByteArray, slotIndex: Int): CiphertextSlot {
        val slotSeed = sha512(SLOT_DOMAIN, seed, littleEndianUInt64((slotIndex.toLong() and 0xffff_ffffL)))
        val u = sampleSmallPolynomial(params, DeterministicStream(slotSeed, U_DOMAIN))
        val e1 = sampleSmallPolynomial(params, DeterministicStream(slotSeed, E1_DOMAIN))
        val e2 = sampleSmallPolynomial(params, DeterministicStream(slotSeed, E2_DOMAIN))
        val encoded = zeroPolynomial(params.polynomialDegree)
        encoded[0] = BigInteger.valueOf(scalar).multiply(params.delta).mod(params.ciphertextModulus)
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

    private fun encodeEnvelopePayload(slots: List<CiphertextSlot>): ByteArray {
        val payload = ByteArrayOutputStream()
        writeField(payload, encodeVecSlots(slots))
        return payload.toByteArray()
    }

    private fun encodeVecSlots(slots: List<CiphertextSlot>): ByteArray {
        val out = ByteArrayOutputStream()
        writeUInt64(out, slots.size.toLong())
        for (slot in slots) {
            val payload = encodeSlot(slot)
            writeUInt64(out, payload.size.toLong())
            out.write(payload)
        }
        return out.toByteArray()
    }

    private fun encodeSlot(slot: CiphertextSlot): ByteArray {
        val out = ByteArrayOutputStream()
        writeField(out, encodeVecU64(slot.c0))
        writeField(out, encodeVecU64(slot.c1))
        return out.toByteArray()
    }

    private fun encodeVecU64(values: Array<BigInteger>): ByteArray {
        val out = ByteArrayOutputStream()
        writeUInt64(out, values.size.toLong())
        for (value in values) {
            val payload = littleEndianUInt64(toUnsignedLong(value))
            writeUInt64(out, payload.size.toLong())
            out.write(payload)
        }
        return out.toByteArray()
    }

    private fun frameNorito(payload: ByteArray): ByteArray {
        val out = ByteArrayOutputStream()
        out.write(NORITO_MAGIC)
        out.write(0)
        out.write(0)
        out.write(schemaHash(SCHEMA_NAME))
        out.write(0)
        out.write(littleEndianUInt64(payload.size.toLong()))
        out.write(littleEndianUInt64(crc64(payload)))
        out.write(0)
        out.write(payload)
        return out.toByteArray()
    }

    private fun writeField(out: ByteArrayOutputStream, payload: ByteArray) {
        writeUInt64(out, payload.size.toLong())
        out.write(payload)
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

    private fun schemaHash(typeName: String): ByteArray {
        var hash = FNV_OFFSET
        for (value in typeName.toByteArray(StandardCharsets.UTF_8)) {
            hash = hash xor (value.toLong() and 0xffL)
            hash *= FNV_PRIME
        }
        val part = littleEndianUInt64(hash)
        val output = ByteArray(16)
        System.arraycopy(part, 0, output, 0, 8)
        System.arraycopy(part, 0, output, 8, 8)
        return output
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

    private class ValidatedParameters(
        val polynomialDegree: Int,
        val ciphertextModulus: BigInteger,
        val delta: BigInteger,
        val maxInputBytes: Int,
        val publicKeyA: Array<BigInteger>,
        val publicKeyB: Array<BigInteger>
    )

    private class CiphertextSlot(val c0: Array<BigInteger>, val c1: Array<BigInteger>)
}
