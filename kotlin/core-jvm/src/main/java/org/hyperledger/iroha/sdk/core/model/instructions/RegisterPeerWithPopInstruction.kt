package org.hyperledger.iroha.sdk.core.model.instructions

import java.util.Base64

private const val ACTION = "RegisterPeerWithPop"

/**
 * Typed representation of a `RegisterPeerWithPop` instruction.
 *
 * The instruction carries a peer identifier (public key) alongside a BLS proof-of-possession.
 * The proof is stored as raw bytes and serialised via base64 in the Norito arguments, mirroring
 * the wire format expected by the Rust data model.
 */
class RegisterPeerWithPopInstruction private constructor(
    @JvmField val peerPublicKey: String,
    private val _proofOfPossession: ByteArray,
    override val arguments: Map<String, String>,
) : InstructionTemplate {

    override val kind: InstructionKind get() = InstructionKind.REGISTER

    fun proofOfPossession(): ByteArray = _proofOfPossession.clone()

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is RegisterPeerWithPopInstruction) return false
        return peerPublicKey == other.peerPublicKey
            && _proofOfPossession.contentEquals(other._proofOfPossession)
    }

    override fun hashCode(): Int {
        var result = peerPublicKey.hashCode()
        result = 31 * result + _proofOfPossession.contentHashCode()
        return result
    }

    class Builder internal constructor() {
        private var peerPublicKey: String? = null
        private var proofOfPossession: ByteArray? = null

        fun setPeerPublicKey(peerPublicKey: String) = apply {
            require(peerPublicKey.isNotBlank()) { "peerPublicKey must not be blank" }
            this.peerPublicKey = peerPublicKey
        }

        fun setProofOfPossession(proofOfPossession: ByteArray) = apply {
            require(proofOfPossession.isNotEmpty()) { "proofOfPossession must not be empty" }
            this.proofOfPossession = proofOfPossession.clone()
        }

        fun build(): RegisterPeerWithPopInstruction {
            val key = checkNotNull(peerPublicKey) { "peerPublicKey must be provided" }
            val pop = checkNotNull(proofOfPossession) { "proofOfPossession must be provided" }
            check(pop.isNotEmpty()) { "proofOfPossession must be provided" }
            val args = buildMap {
                put("action", ACTION)
                put("peer", key)
                put("pop", Base64.getEncoder().encodeToString(pop))
            }
            return RegisterPeerWithPopInstruction(key, pop.clone(), args)
        }
    }

    companion object {
        @JvmStatic
        fun builder(): Builder = Builder()

        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): RegisterPeerWithPopInstruction {
            val peerKey = requireArg(arguments, "peer")
            val pop = decodeBase64(requireArg(arguments, "pop"))
            return RegisterPeerWithPopInstruction(peerKey, pop, LinkedHashMap(arguments))
        }

        private fun requireArg(arguments: Map<String, String>, key: String): String {
            val value = arguments[key]
            require(!value.isNullOrBlank()) { "Instruction argument '$key' is required" }
            return value
        }

        private fun decodeBase64(value: String): ByteArray = try {
            Base64.getDecoder().decode(value)
        } catch (ex: IllegalArgumentException) {
            throw IllegalArgumentException("pop must be base64 encoded", ex)
        }
    }
}
