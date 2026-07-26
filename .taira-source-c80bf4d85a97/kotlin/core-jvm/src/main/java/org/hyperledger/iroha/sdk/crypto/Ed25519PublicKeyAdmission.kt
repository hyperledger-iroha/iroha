package org.hyperledger.iroha.sdk.crypto

import java.math.BigInteger
import org.bouncycastle.math.ec.rfc8032.Ed25519

/** Strict admission checks for canonical prime-order Ed25519 public keys. */
object Ed25519PublicKeyAdmission {
    /** Canonical compressed Ed25519 public-key length. */
    const val PUBLIC_KEY_LENGTH: Int = 32

    private val TWO = BigInteger.valueOf(2L)
    private val FIELD_MODULUS = BigInteger.ONE.shiftLeft(255).subtract(BigInteger.valueOf(19L))
    private val SUBGROUP_ORDER =
        BigInteger.ONE.shiftLeft(252)
            .add(BigInteger("27742317777372353535851937790883648493"))
    private val CURVE_D =
        BigInteger.valueOf(-121665L)
            .multiply(BigInteger.valueOf(121666L).modInverse(FIELD_MODULUS))
            .mod(FIELD_MODULUS)
    private val TWO_CURVE_D = TWO.multiply(CURVE_D).mod(FIELD_MODULUS)
    private val SQRT_MINUS_ONE =
        TWO.modPow(FIELD_MODULUS.subtract(BigInteger.ONE).shiftRight(2), FIELD_MODULUS)
    private val SQRT_EXPONENT =
        FIELD_MODULUS.add(BigInteger.valueOf(3L)).shiftRight(3)
    private val EXTENDED_IDENTITY =
        ExtendedPoint(BigInteger.ZERO, BigInteger.ONE, BigInteger.ONE, BigInteger.ZERO)

    /** Returns `true` only for canonical points in the prime-order Ed25519 subgroup. */
    @JvmStatic
    fun isValid(publicKey: ByteArray?): Boolean {
        if (publicKey == null || publicKey.size != PUBLIC_KEY_LENGTH) return false

        val encoded = publicKey.copyOf()
        // Bouncy Castle's partial validator is only a curve/decompression
        // prefilter. Prime-order admission is enforced explicitly below so
        // mixed-torsion points cannot pass through provider-specific behavior.
        if (!Ed25519.validatePublicKeyPartial(encoded, 0)) return false
        val point = decodeCanonicalPoint(encoded) ?: return false
        if (isIdentity(point)) return false
        return isIdentity(scalarMultiply(point, SUBGROUP_ORDER))
    }

    private fun decodeCanonicalPoint(encoded: ByteArray): ExtendedPoint? {
        val sign = (encoded[PUBLIC_KEY_LENGTH - 1].toInt() ushr 7) and 1
        val yBytes = encoded.copyOf()
        yBytes[PUBLIC_KEY_LENGTH - 1] =
            (yBytes[PUBLIC_KEY_LENGTH - 1].toInt() and 0x7F).toByte()
        val y = decodeLittleEndian(yBytes)
        if (y >= FIELD_MODULUS) return null

        val ySquared = mod(y.multiply(y))
        val numerator = mod(ySquared.subtract(BigInteger.ONE))
        val denominator = mod(CURVE_D.multiply(ySquared).add(BigInteger.ONE))
        val xSquared = try {
            mod(numerator.multiply(denominator.modInverse(FIELD_MODULUS)))
        } catch (_: ArithmeticException) {
            return null
        }
        var x = xSquared.modPow(SQRT_EXPONENT, FIELD_MODULUS)
        if (mod(x.multiply(x)) != xSquared) {
            x = mod(x.multiply(SQRT_MINUS_ONE))
        }
        if (mod(x.multiply(x)) != xSquared) return null
        if ((if (x.testBit(0)) 1 else 0) != sign) {
            x = FIELD_MODULUS.subtract(x).mod(FIELD_MODULUS)
        }
        if (x.signum() == 0 && sign == 1) return null
        if (!encodeCanonicalPoint(x, y).contentEquals(encoded)) return null

        return ExtendedPoint(x, y, BigInteger.ONE, mod(x.multiply(y)))
    }

    private fun encodeCanonicalPoint(x: BigInteger, y: BigInteger): ByteArray {
        val encoded = encodeLittleEndian(y, PUBLIC_KEY_LENGTH)
        if (x.testBit(0)) {
            encoded[PUBLIC_KEY_LENGTH - 1] =
                (encoded[PUBLIC_KEY_LENGTH - 1].toInt() or 0x80).toByte()
        }
        return encoded
    }

    private fun decodeLittleEndian(encoded: ByteArray): BigInteger =
        BigInteger(1, encoded.reversedArray())

    private fun encodeLittleEndian(value: BigInteger, size: Int): ByteArray {
        val bigEndian = value.toByteArray()
        val encoded = ByteArray(size)
        for (index in 0 until minOf(size, bigEndian.size)) {
            encoded[index] = bigEndian[bigEndian.size - 1 - index]
        }
        return encoded
    }

    private fun scalarMultiply(point: ExtendedPoint, scalar: BigInteger): ExtendedPoint {
        var result = EXTENDED_IDENTITY
        var addend = point
        var value = scalar
        while (value.signum() != 0) {
            if (value.testBit(0)) {
                result = add(result, addend)
            }
            addend = add(addend, addend)
            value = value.shiftRight(1)
        }
        return result
    }

    private fun add(left: ExtendedPoint, right: ExtendedPoint): ExtendedPoint {
        val a = mod(left.y.subtract(left.x).multiply(right.y.subtract(right.x)))
        val b = mod(left.y.add(left.x).multiply(right.y.add(right.x)))
        val c = mod(TWO_CURVE_D.multiply(left.t).multiply(right.t))
        val d = mod(TWO.multiply(left.z).multiply(right.z))
        val e = mod(b.subtract(a))
        val f = mod(d.subtract(c))
        val g = mod(d.add(c))
        val h = mod(b.add(a))
        return ExtendedPoint(
            mod(e.multiply(f)),
            mod(g.multiply(h)),
            mod(f.multiply(g)),
            mod(e.multiply(h)),
        )
    }

    private fun isIdentity(point: ExtendedPoint): Boolean {
        val z = mod(point.z)
        return z.signum() != 0 &&
            mod(point.x).signum() == 0 &&
            mod(point.y) == z
    }

    private fun mod(value: BigInteger): BigInteger = value.mod(FIELD_MODULUS)

    private data class ExtendedPoint(
        val x: BigInteger,
        val y: BigInteger,
        val z: BigInteger,
        val t: BigInteger,
    )
}
