package org.hyperledger.iroha.sdk.core.model.instructions

private const val ACTION = "PublishSpaceDirectoryManifest"

/**
 * Typed representation of a `PublishSpaceDirectoryManifest` instruction.
 *
 * The manifest payload is stored as canonical JSON so Android callers can reuse the same
 * fixtures and workflow described in `docs/space-directory.md` without inventing bespoke
 * encodings.
 */
class PublishSpaceDirectoryManifestInstruction(
    @JvmField val manifestJson: String,
) : InstructionTemplate {

    init {
        require(manifestJson.isNotBlank()) { "manifestJson must not be blank" }
    }

    override val kind: InstructionKind = InstructionKind.CUSTOM

    override val arguments: Map<String, String> by lazy {
        linkedMapOf(
            "action" to ACTION,
            "manifest_json" to manifestJson,
        )
    }

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is PublishSpaceDirectoryManifestInstruction) return false
        return manifestJson == other.manifestJson
    }

    override fun hashCode(): Int = manifestJson.hashCode()

    companion object {
        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): PublishSpaceDirectoryManifestInstruction {
            val manifestJson = requireArg(arguments, "manifest_json")
            return PublishSpaceDirectoryManifestInstruction(manifestJson = manifestJson)
        }

        private fun requireArg(arguments: Map<String, String>, key: String): String {
            val value = arguments[key]
            if (value.isNullOrBlank()) {
                throw IllegalArgumentException("Instruction argument '$key' is required")
            }
            return value
        }
    }
}
