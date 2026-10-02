package org.hyperledger.iroha.sdk.offline.petal

/**
 * Marsaglia xorshift32 with shifts 13, 17 and 5.
 *
 * The generator is part of the Petal wire format: lane whitening sequences are
 * derived from it, so every implementation must match bit for bit. A zero seed
 * is replaced by `0xDEADBEEF` because xorshift cannot leave the all-zero state.
 * Instances are mutable and not thread-safe.
 */
class PetalXorshift32(seed: Int) {
    private var state = if (seed == 0) 0xDEAD_BEEF.toInt() else seed

    /** Advances the generator and returns the next 32-bit word (raw bits). */
    fun nextInt(): Int {
        var x = state
        x = x xor (x shl 13)
        x = x xor (x ushr 17)
        x = x xor (x shl 5)
        state = x
        return x
    }

    /** Returns the top byte (`0..255`) of the next word. */
    fun nextByte(): Int = nextInt() ushr 24

    override fun equals(other: Any?): Boolean = other is PetalXorshift32 && state == other.state

    override fun hashCode(): Int = state
}
