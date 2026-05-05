package org.hyperledger.iroha.sdk.core.model.instructions

private const val ACTION = "RegisterNft"

/** Typed representation of `RegisterNft` instructions. */
class RegisterNftInstruction(
    @JvmField val nftId: String,
    metadata: Map<String, String> = emptyMap(),
    arguments: Map<String, String>? = null,
) : InstructionTemplate {

    private val _metadata: Map<String, String> = metadata.toMap()
    private val _arguments: Map<String, String> = arguments?.toMap() ?: canonicalArguments()

    val metadata: Map<String, String> get() = _metadata

    override val kind: InstructionKind = InstructionKind.REGISTER

    override val arguments: Map<String, String> get() = _arguments

    init {
        require(nftId.isNotBlank()) { "nftId must not be blank" }
    }

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is RegisterNftInstruction) return false
        return nftId == other.nftId && _metadata == other._metadata
    }

    override fun hashCode(): Int = listOf(nftId, _metadata).hashCode()

    private fun canonicalArguments(): Map<String, String> = buildMap {
        put("action", ACTION)
        put("nft", nftId)
        _metadata.forEach { (key, value) -> put("metadata.$key", value) }
    }

    companion object {
        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): RegisterNftInstruction {
            val metadata = buildMap {
                arguments.forEach { (key, value) ->
                    if (key.startsWith("metadata.")) {
                        val metadataKey = key.removePrefix("metadata.")
                        if (metadataKey.isNotEmpty()) {
                            put(metadataKey, value)
                        }
                    }
                }
            }
            return RegisterNftInstruction(
                nftId = requireArgument(arguments, "nft"),
                metadata = metadata,
                arguments = arguments,
            )
        }

        private fun requireArgument(arguments: Map<String, String>, key: String): String {
            val value = arguments[key]
            require(!value.isNullOrBlank()) { "Instruction argument '$key' is required" }
            return value
        }
    }
}
