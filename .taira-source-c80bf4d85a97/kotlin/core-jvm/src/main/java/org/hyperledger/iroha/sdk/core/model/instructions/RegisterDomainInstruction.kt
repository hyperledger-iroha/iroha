package org.hyperledger.iroha.sdk.core.model.instructions

private const val ACTION = "RegisterDomain"

/** Typed representation of the `RegisterDomain` instruction. */
class RegisterDomainInstruction(
    @JvmField val domainName: String,
    @JvmField val logo: String? = null,
    metadata: Map<String, String> = emptyMap(),
    arguments: Map<String, String>? = null,
) : InstructionTemplate {

    private val _metadata: Map<String, String> = metadata.toMap()
    private val _arguments: Map<String, String> = arguments?.toMap() ?: canonicalArguments()

    val metadata: Map<String, String> get() = _metadata

    override val kind: InstructionKind = InstructionKind.REGISTER

    override val arguments: Map<String, String> get() = _arguments

    init {
        require(domainName.isNotBlank()) { "domainName must not be blank" }
    }

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is RegisterDomainInstruction) return false
        return domainName == other.domainName
            && logo == other.logo
            && _metadata == other._metadata
    }

    override fun hashCode(): Int = listOf(domainName, logo, _metadata).hashCode()

    private fun canonicalArguments(): Map<String, String> = buildMap {
        put("action", ACTION)
        put("domain", domainName)
        if (logo != null) put("logo", logo)
        _metadata.forEach { (key, value) -> put("metadata.$key", value) }
    }

    companion object {
        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): RegisterDomainInstruction {
            val metadata = buildMap {
                arguments.forEach { (key, value) ->
                    if (key.startsWith("metadata.")) {
                        put(key.removePrefix("metadata."), value)
                    }
                }
            }
            return RegisterDomainInstruction(
                domainName = requireArgument(arguments, "domain"),
                logo = arguments["logo"],
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
