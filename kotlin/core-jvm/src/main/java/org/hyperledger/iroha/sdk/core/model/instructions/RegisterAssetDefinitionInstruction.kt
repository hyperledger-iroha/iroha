package org.hyperledger.iroha.sdk.core.model.instructions

private const val ACTION = "RegisterAssetDefinition"

/** Typed representation of the `RegisterAssetDefinition` instruction. */
class RegisterAssetDefinitionInstruction(
    @JvmField val assetDefinitionId: String,
    @JvmField val displayName: String? = null,
    @JvmField val description: String? = null,
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
        require(assetDefinitionId.isNotBlank()) { "assetDefinitionId must not be blank" }
    }

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is RegisterAssetDefinitionInstruction) return false
        return assetDefinitionId == other.assetDefinitionId
            && displayName == other.displayName
            && description == other.description
            && logo == other.logo
            && _metadata == other._metadata
    }

    override fun hashCode(): Int =
        listOf(assetDefinitionId, displayName, description, logo, _metadata).hashCode()

    private fun canonicalArguments(): Map<String, String> = buildMap {
        put("action", ACTION)
        put("definition", assetDefinitionId)
        if (displayName != null) put("display_name", displayName)
        if (description != null) put("description", description)
        if (logo != null) put("logo", logo)
        _metadata.forEach { (key, value) -> put("metadata.$key", value) }
    }

    companion object {
        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): RegisterAssetDefinitionInstruction {
            val metadata = buildMap {
                arguments.forEach { (key, value) ->
                    if (key.startsWith("metadata.")) {
                        put(key.removePrefix("metadata."), value)
                    }
                }
            }
            return RegisterAssetDefinitionInstruction(
                assetDefinitionId = requireArgument(arguments, "definition"),
                displayName = arguments["display_name"],
                description = arguments["description"],
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
