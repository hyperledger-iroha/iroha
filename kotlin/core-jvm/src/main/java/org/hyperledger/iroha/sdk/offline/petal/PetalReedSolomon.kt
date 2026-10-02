package org.hyperledger.iroha.sdk.offline.petal

/**
 * GF(2^8) arithmetic over the primitive polynomial `x^8 + x^4 + x^3 + x^2 + 1`
 * (`0x11D`, the QR Code field) with `α = 2`. Elements are `Int`s in `0..255`.
 */
internal object PetalGf256 {
    private const val PRIMITIVE = 0x11D

    val EXP = IntArray(512)
    val LOG = IntArray(256)

    init {
        var x = 1
        for (i in 0 until 255) {
            EXP[i] = x
            LOG[x] = i
            x = x shl 1
            if ((x and 0x100) != 0) x = x xor PRIMITIVE
        }
        for (j in 255 until 512) EXP[j] = EXP[j - 255]
    }

    fun mul(a: Int, b: Int): Int = if (a == 0 || b == 0) 0 else EXP[LOG[a] + LOG[b]]

    /** Divides `a` by a non-zero `b`. */
    fun div(a: Int, b: Int): Int = if (a == 0) 0 else EXP[LOG[a] + 255 - LOG[b]]

    fun exp(exponent: Int): Int = EXP[exponent % 255]

    /** Multiplicative inverse of a non-zero element. */
    fun inv(a: Int): Int = EXP[255 - LOG[a]]

    /** Multiplies two lowest-degree-first polynomials. */
    fun polyMul(a: IntArray, b: IntArray): IntArray {
        val out = IntArray(a.size + b.size - 1)
        for (i in a.indices) {
            val x = a[i]
            if (x == 0) continue
            for (j in b.indices) out[i + j] = out[i + j] xor mul(x, b[j])
        }
        return out
    }

    /** Evaluates a lowest-degree-first polynomial at `x` (Horner). */
    fun polyEval(poly: IntArray, x: Int): Int {
        var acc = 0
        for (index in poly.size - 1 downTo 0) acc = mul(acc, x) xor poly[index]
        return acc
    }
}

/** Why a Reed–Solomon decode failed. */
class PetalReedSolomonException(
    /** The failure class. */
    val reason: Reason,
) : IllegalArgumentException(reason.description) {
    /** Reed–Solomon failure classes. */
    enum class Reason(internal val description: String) {
        /** The codeword length, parity count or erasure list is invalid. */
        INVALID_SHAPE("invalid Reed-Solomon codeword shape"),

        /** More errata than the code can correct, or the word is not decodable. */
        UNCORRECTABLE("Reed-Solomon word is uncorrectable"),
    }
}

/**
 * Reed–Solomon over GF(2^8) with errors-and-erasures decoding.
 *
 * A codeword is `data || parity`, systematic; the generator is
 * `g(x) = (x - α^0)(x - α^1)…(x - α^(nsym-1))`, so the first consecutive root
 * is `α^0`. The first byte of a codeword is the highest-degree coefficient.
 * Every Petal lane is one codeword (`n <= 255`); low-confidence cells are
 * passed to [decode] as erasures, which cost one parity symbol each instead of
 * two. Instances are immutable and thread-safe.
 */
class PetalReedSolomon(
    /** Number of parity bytes (`1..254`). */
    val parityLength: Int,
) {
    /** Highest-degree-first monic generator polynomial. */
    private val generator: IntArray

    init {
        require(parityLength in 1..254) { "Reed-Solomon parity byte count out of range" }
        var polynomial = intArrayOf(1)
        for (i in 0 until parityLength) {
            val root = PetalGf256.exp(i)
            val next = IntArray(polynomial.size + 1)
            for (k in polynomial.indices) {
                next[k] = next[k] xor polynomial[k]
                next[k + 1] = next[k + 1] xor PetalGf256.mul(polynomial[k], root)
            }
            polynomial = next
        }
        generator = polynomial
    }

    /** Encodes [data], returning `data || parity`. */
    fun encode(data: ByteArray): ByteArray {
        require(data.size + parityLength <= 255) { "Reed-Solomon codeword longer than 255 bytes" }
        val remainder = IntArray(parityLength)
        for (byte in data) {
            val feedback = (byte.toInt() and 0xFF) xor remainder[0]
            for (j in 0 until parityLength) {
                val next = if (j + 1 < parityLength) remainder[j + 1] else 0
                remainder[j] = next xor PetalGf256.mul(feedback, generator[j + 1])
            }
        }
        val word = data.copyOf(data.size + parityLength)
        for (j in 0 until parityLength) word[data.size + j] = remainder[j].toByte()
        return word
    }

    /**
     * Corrects [word] in place, treating [erasures] as known-bad positions.
     *
     * Succeeds when `2 * errors + erasures <= parityLength`. The corrected word
     * is re-checked against zero syndromes before it is written back, so a
     * success always yields a valid codeword; on failure [word] is unchanged.
     *
     * @return the number of corrected positions.
     * @throws PetalReedSolomonException when the arguments are malformed or the
     *   word cannot be decoded.
     */
    @JvmOverloads
    fun decode(word: ByteArray, erasures: IntArray = IntArray(0)): Int {
        val result = decodeInPlace(word, erasures, erasures.size)
        if (result >= 0) return result
        throw PetalReedSolomonException(
            if (result == INVALID_SHAPE) {
                PetalReedSolomonException.Reason.INVALID_SHAPE
            } else {
                PetalReedSolomonException.Reason.UNCORRECTABLE
            },
        )
    }

    /** Syndromes of [word] as an int vector. */
    internal fun syndromes(word: IntArray): IntArray = IntArray(parityLength) { j ->
        val root = PetalGf256.exp(j)
        var acc = 0
        for (value in word) acc = PetalGf256.mul(acc, root) xor value
        acc
    }

    /**
     * Allocation-light decode used by the lane codecs: returns the number of
     * corrected positions, [INVALID_SHAPE] or [UNCORRECTABLE]. Only the first
     * [erasureCount] entries of [erasures] are used.
     */
    internal fun decodeInPlace(word: ByteArray, erasures: IntArray, erasureCount: Int): Int {
        val n = word.size
        if (n <= parityLength || n > 255 || erasureCount > parityLength) return INVALID_SHAPE
        val seen = BooleanArray(255)
        for (index in 0 until erasureCount) {
            val position = erasures[index]
            if (position < 0 || position >= n || seen[position]) return INVALID_SHAPE
            seen[position] = true
        }
        val received = IntArray(n) { word[it].toInt() and 0xFF }
        val syndromes = syndromes(received)
        if (syndromes.all { it == 0 }) return 0
        val f = erasureCount
        // Erasure locator Γ(x) = Π (1 + X_e x), lowest degree first.
        var gamma = intArrayOf(1)
        for (index in 0 until erasureCount) {
            val x = PetalGf256.exp(n - 1 - erasures[index])
            gamma = PetalGf256.polyMul(gamma, intArrayOf(1, x))
        }
        // Forney syndromes: the coefficients of S(x)Γ(x) from index f upward
        // are the syndromes of the error-only word.
        val forney = PetalGf256.polyMul(syndromes, gamma).copyOf(parityLength)
        val lambda = berlekampMassey(forney.copyOfRange(f, parityLength))
        val errorCount = lambda.size - 1
        if (2 * errorCount + f > parityLength) return UNCORRECTABLE
        val psi = PetalGf256.polyMul(lambda, gamma)
        val degree = psi.size - 1
        // Chien search over all positions.
        val positions = IntArray(n)
        var found = 0
        for (i in 0 until n) {
            val xInverse = PetalGf256.exp(255 - ((n - 1 - i) % 255))
            if (PetalGf256.polyEval(psi, xInverse) == 0) positions[found++] = i
        }
        if (found != degree) return UNCORRECTABLE
        // Ω(x) = S(x)Ψ(x) mod x^nsym.
        val omega = PetalGf256.polyMul(syndromes, psi).let { if (it.size > parityLength) it.copyOf(parityLength) else it }
        // Formal derivative of Ψ in characteristic 2 keeps odd-degree terms.
        val derivative = IntArray(psi.size - 1) { k -> if ((k + 1) % 2 == 1) psi[k + 1] else 0 }
        val corrected = received.copyOf()
        for (index in 0 until found) {
            val i = positions[index]
            val x = PetalGf256.exp(n - 1 - i)
            val xInverse = PetalGf256.inv(x)
            val numerator = PetalGf256.polyEval(omega, xInverse)
            val denominator = PetalGf256.polyEval(derivative, xInverse)
            if (denominator == 0) return UNCORRECTABLE
            corrected[i] = corrected[i] xor PetalGf256.mul(x, PetalGf256.div(numerator, denominator))
        }
        if (syndromes(corrected).any { it != 0 }) return UNCORRECTABLE
        for (i in 0 until n) word[i] = corrected[i].toByte()
        return found
    }

    internal companion object {
        const val INVALID_SHAPE = -1
        const val UNCORRECTABLE = -2

        /** Berlekamp–Massey over GF(256); returns the lowest-degree-first locator. */
        fun berlekampMassey(syndromes: IntArray): IntArray {
            val n = syndromes.size
            var c = IntArray(n + 1)
            var b = IntArray(n + 1)
            c[0] = 1
            b[0] = 1
            var l = 0
            var m = 1
            var previousDiscrepancy = 1
            for (i in 0 until n) {
                var d = syndromes[i]
                for (j in 1..l) d = d xor PetalGf256.mul(c[j], syndromes[i - j])
                if (d == 0) {
                    m += 1
                    continue
                }
                val scale = PetalGf256.div(d, previousDiscrepancy)
                if (2 * l <= i) {
                    val snapshot = c.copyOf()
                    for (j in 0 until maxOf(n + 1 - m, 0)) c[j + m] = c[j + m] xor PetalGf256.mul(scale, b[j])
                    l = i + 1 - l
                    b = snapshot
                    previousDiscrepancy = d
                    m = 1
                } else {
                    for (j in 0 until maxOf(n + 1 - m, 0)) c[j + m] = c[j + m] xor PetalGf256.mul(scale, b[j])
                    m += 1
                }
            }
            c = c.copyOf(l + 1)
            return c
        }
    }
}
