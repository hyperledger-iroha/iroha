package org.hyperledger.iroha.sdk.core.model.instructions

private const val ACTION = "RegisterAccount"

/** Typed representation of the `RegisterAccount` instruction. */
class RegisterAccountInstruction(
    @JvmField val accountId: String,
    metadata: Map<String, String> = emptyMap(),
    @JvmField val uaid: String? = null,
    arguments: Map<String, String>? = null,
) : InstructionTemplate {

    private val _metadata: Map<String, String> = metadata.toMap()
    private val _arguments: Map<String, String> = arguments?.toMap() ?: canonicalArguments()

    val metadata: Map<String, String> get() = _metadata

    override val kind: InstructionKind = InstructionKind.REGISTER

    override val arguments: Map<String, String> get() = _arguments

    init {
        require(accountId.isNotBlank()) { "accountId must not be blank" }
    }

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is RegisterAccountInstruction) return false
        return accountId == other.accountId
            && _metadata == other._metadata
            && uaid == other.uaid
    }

    override fun hashCode(): Int = listOf(accountId, _metadata, uaid).hashCode()

    private fun canonicalArguments(): Map<String, String> = buildMap {
        put("action", ACTION)
        put("account", accountId)
        if (!uaid.isNullOrBlank()) put("uaid", uaid)
        _metadata.forEach { (key, value) -> put("metadata.$key", value) }
    }

    companion object {
        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): RegisterAccountInstruction {
            val metadata = buildMap {
                arguments.forEach { (key, value) ->
                    if (key.startsWith("metadata.")) {
                        put(key.removePrefix("metadata."), value)
                    }
                }
            }
            return RegisterAccountInstruction(
                accountId = requireArgument(arguments, "account"),
                metadata = metadata,
                uaid = arguments["uaid"],
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
