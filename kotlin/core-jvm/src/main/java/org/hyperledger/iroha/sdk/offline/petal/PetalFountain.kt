package org.hyperledger.iroha.sdk.offline.petal

/**
 * Rateless fountain code over GF(2).
 *
 * A payload is cut into `k` source atoms of [PetalLanes.ATOM_LEN] bytes (the
 * last one zero-padded). Encoded atom `id` is source atom `id` for `id < k`
 * (systematic), and otherwise the XOR of a pseudo-random half of the source
 * atoms chosen by [maskWords]. A receiver holding any `k + 2` or so independent
 * atoms, in any order, recovers the payload by Gaussian elimination; lost frames
 * cost nothing but time. Atom ids and CRCs are raw unsigned 32-bit patterns.
 */
object PetalFountain {
    private const val ATOM = PetalLanes.ATOM_LEN

    /** Splits a payload into zero-padded source atoms. */
    @JvmStatic
    fun splitPayload(payload: ByteArray): List<ByteArray> {
        val count = (payload.size + ATOM - 1) / ATOM
        return List(count) { index ->
            ByteArray(ATOM).also { atom ->
                val start = index * ATOM
                payload.copyInto(atom, 0, start, minOf(start + ATOM, payload.size))
            }
        }
    }

    /** Number of 32-bit words needed for a mask over [k] source atoms. */
    @JvmStatic
    fun maskLength(k: Int): Int {
        require(k >= 0) { "source atom count must not be negative" }
        return (k + 31) / 32
    }

    /**
     * The 32-bit finalizer of MurmurHash3 (`fmix32`).
     *
     * Masks must not come from a GF(2)-linear generator such as xorshift: every
     * mask would then lie in a subspace of dimension at most 32. The
     * multiplications make this mixer nonlinear over GF(2).
     */
    @JvmStatic
    fun mix32(value: Int): Int {
        var x = value
        x = x xor (x ushr 16)
        x *= 0x85EB_CA6B.toInt()
        x = x xor (x ushr 13)
        x *= 0xC2B2_AE35.toInt()
        x = x xor (x ushr 16)
        return x
    }

    /**
     * The combination mask of encoded atom [id] as little-endian bit words.
     *
     * [crc] is the payload CRC-32C and only diversifies masks between streams.
     * Atoms with `id < k` are systematic (a unit vector); every other atom
     * combines a pseudo-random half of the sources:
     *
     * ```text
     * seed    = mix32((id * 0x9E3779B1) ^ crc ^ 0xA5A5A5A5)
     * word[w] = mix32(seed + (w + 1) * 0x9E3779B9)      (all arithmetic mod 2^32)
     * ```
     *
     * Bits at or above `k` are cleared, and an all-zero mask is replaced by the
     * single bit `id mod k`.
     */
    @JvmStatic
    fun maskWords(k: Int, crc: Int, id: Int): IntArray {
        require(k >= 1) { "a stream has at least one source atom" }
        val mask = IntArray(maskLength(k))
        fillMask(mask, k, crc, id)
        return mask
    }

    /** Encodes atom [id] from the [source] atoms (each [PetalLanes.ATOM_LEN] bytes). */
    @JvmStatic
    fun encodeAtom(source: List<ByteArray>, crc: Int, id: Int): ByteArray {
        require(source.isNotEmpty()) { "a stream has at least one source atom" }
        val flat = ByteArray(source.size * ATOM)
        source.forEachIndexed { index, atom ->
            require(atom.size == ATOM) { "atoms are ${PetalLanes.ATOM_LEN} bytes" }
            atom.copyInto(flat, index * ATOM)
        }
        return ByteArray(ATOM).also { encodeAtomInto(flat, source.size, crc, id, IntArray(maskLength(source.size)), it, 0) }
    }

    /** Writes the mask of atom [id] over [k] sources into [mask] (`maskLength(k)` words). */
    internal fun fillMask(mask: IntArray, k: Int, crc: Int, id: Int) {
        java.util.Arrays.fill(mask, 0)
        val unsignedId = id.toLong() and 0xFFFF_FFFFL
        if (unsignedId < k) {
            mask[id / 32] = 1 shl (id % 32)
            return
        }
        val seed = mix32((id * 0x9E37_79B1.toInt()) xor crc xor 0xA5A5_A5A5.toInt())
        for (w in mask.indices) mask[w] = mix32(seed + (w + 1) * 0x9E37_79B9.toInt())
        val tail = k % 32
        if (tail != 0) mask[mask.size - 1] = mask[mask.size - 1] and ((1 shl tail) - 1)
        if (mask.all { it == 0 }) {
            val bit = (unsignedId % k).toInt()
            mask[bit / 32] = mask[bit / 32] or (1 shl (bit % 32))
        }
    }

    /**
     * XORs the sources selected by atom [id] into `out[offset until offset + 16]`
     * (which must start zeroed). [source] holds [k] atoms back to back and
     * [mask] is scratch space of `maskLength(k)` words.
     */
    internal fun encodeAtomInto(source: ByteArray, k: Int, crc: Int, id: Int, mask: IntArray, out: ByteArray, offset: Int) {
        fillMask(mask, k, crc, id)
        for (index in 0 until k) {
            if (((mask[index / 32] ushr (index % 32)) and 1) == 1) {
                val base = index * ATOM
                for (byte in 0 until ATOM) {
                    out[offset + byte] = (out[offset + byte].toInt() xor source[base + byte].toInt()).toByte()
                }
            }
        }
    }
}

/**
 * Incremental Gaussian-elimination decoder for [PetalFountain] atoms.
 * Instances are mutable and not thread-safe.
 */
class PetalFountainDecoder(k: Int) {
    /** Number of source atoms. */
    val sourceAtoms: Int = k

    private val words: Int
    private val pivot: IntArray
    private val masks = ArrayList<IntArray>()
    private val data = ArrayList<ByteArray>()

    init {
        require(k > 0) { "a stream has at least one source atom" }
        words = PetalFountain.maskLength(k)
        pivot = IntArray(k) { -1 }
    }

    /** Number of linearly independent atoms received so far. */
    val rank: Int get() = masks.size

    /** Whether enough independent atoms arrived to recover the payload. */
    val isComplete: Boolean get() = masks.size == sourceAtoms

    /** Adds encoded atom [id]; returns whether it increased the rank. */
    fun addEncoded(crc: Int, id: Int, atom: ByteArray): Boolean {
        val mask = IntArray(words)
        PetalFountain.fillMask(mask, sourceAtoms, crc, id)
        return addOwned(mask, checkAtom(atom).copyOf())
    }

    /** Adds a received combination; returns whether it increased the rank. */
    fun add(mask: IntArray, atom: ByteArray): Boolean = addOwned(mask.copyOf(), checkAtom(atom).copyOf())

    internal fun addOwned(mask: IntArray, atom: ByteArray): Boolean {
        if (mask.size != words) return false
        var word = 0
        while (true) {
            while (word < mask.size && mask[word] == 0) word += 1
            if (word == mask.size) return false
            val column = word * 32 + Integer.numberOfTrailingZeros(mask[word])
            if (column >= sourceAtoms) return false
            val row = pivot[column]
            if (row >= 0) {
                val pivotMask = masks[row]
                for (index in word until mask.size) mask[index] = mask[index] xor pivotMask[index]
                val pivotData = data[row]
                for (index in 0 until PetalLanes.ATOM_LEN) atom[index] = (atom[index].toInt() xor pivotData[index].toInt()).toByte()
            } else {
                pivot[column] = masks.size
                masks += mask
                data += atom
                return true
            }
        }
    }

    /** Returns the source atoms once the decoder is complete, else `null`. */
    fun solve(): List<ByteArray>? {
        val flat = solveFlat() ?: return null
        return List(sourceAtoms) { flat.copyOfRange(it * PetalLanes.ATOM_LEN, (it + 1) * PetalLanes.ATOM_LEN) }
    }

    /** The solved source atoms back to back, or `null` when incomplete. */
    internal fun solveFlat(): ByteArray? {
        if (!isComplete) return null
        val atom = PetalLanes.ATOM_LEN
        val solution = ByteArray(sourceAtoms * atom)
        for (column in sourceAtoms - 1 downTo 0) {
            val row = pivot[column]
            if (row < 0) return null
            val mask = masks[row]
            val value = data[row].copyOf()
            val firstWord = column / 32
            for (word in firstWord until mask.size) {
                var bits = mask[word]
                if (word == firstWord) {
                    // keep only columns strictly above the pivot
                    val shift = column % 32 + 1
                    bits = if (shift >= 32) 0 else (bits ushr shift) shl shift
                }
                while (bits != 0) {
                    val other = word * 32 + Integer.numberOfTrailingZeros(bits)
                    bits = bits and (bits - 1)
                    val base = other * atom
                    for (index in 0 until atom) value[index] = (value[index].toInt() xor solution[base + index].toInt()).toByte()
                }
            }
            value.copyInto(solution, column * atom)
        }
        return solution
    }

    private fun checkAtom(atom: ByteArray): ByteArray {
        require(atom.size == PetalLanes.ATOM_LEN) { "atoms are ${PetalLanes.ATOM_LEN} bytes" }
        return atom
    }
}
