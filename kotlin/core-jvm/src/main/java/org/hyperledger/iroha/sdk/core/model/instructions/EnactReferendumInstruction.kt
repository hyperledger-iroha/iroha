package org.hyperledger.iroha.sdk.core.model.instructions

private const val ENACT_REFERENDUM_ACTION = "EnactReferendum"

/** Typed representation of `EnactReferendum` instructions. */
class EnactReferendumInstruction private constructor(
    val referendumIdHex: String,
    val preimageHashHex: String,
    val window: GovernanceInstructionUtils.AtWindow,
    private val _arguments: Map<String, String>,
) : InstructionTemplate {

    override val kind: InstructionKind get() = InstructionKind.CUSTOM

    override val arguments: Map<String, String> get() = _arguments

    constructor(
        referendumIdHex: String,
        preimageHashHex: String,
        window: GovernanceInstructionUtils.AtWindow,
    ) : this(
        referendumIdHex = GovernanceInstructionUtils.requireHex(referendumIdHex, "referendumIdHex", 32),
        preimageHashHex = GovernanceInstructionUtils.requireHex(preimageHashHex, "preimageHashHex", 32),
        window = window,
        _arguments = buildCanonicalArguments(
            GovernanceInstructionUtils.requireHex(referendumIdHex, "referendumIdHex", 32),
            GovernanceInstructionUtils.requireHex(preimageHashHex, "preimageHashHex", 32),
            window,
        ),
    )

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is EnactReferendumInstruction) return false
        return referendumIdHex == other.referendumIdHex
            && preimageHashHex == other.preimageHashHex
            && window == other.window
    }

    override fun hashCode(): Int {
        var result = referendumIdHex.hashCode()
        result = 31 * result + preimageHashHex.hashCode()
        result = 31 * result + (window?.hashCode() ?: 0)
        return result
    }

    companion object {
        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): EnactReferendumInstruction {
            val referendumIdHex = require(arguments, "referendum_id_hex")
            val preimageHashHex = require(arguments, "preimage_hash_hex")
            val window = GovernanceInstructionUtils.parseAtWindow(arguments, "window", "enactment window")
            return EnactReferendumInstruction(
                referendumIdHex = referendumIdHex,
                preimageHashHex = preimageHashHex,
                window = window,
                _arguments = LinkedHashMap(arguments),
            )
        }

        private fun buildCanonicalArguments(
            referendumIdHex: String,
            preimageHashHex: String,
            window: GovernanceInstructionUtils.AtWindow,
        ): Map<String, String> {
            val args = linkedMapOf(
                "action" to ENACT_REFERENDUM_ACTION,
                "referendum_id_hex" to referendumIdHex,
                "preimage_hash_hex" to preimageHashHex,
            )
            GovernanceInstructionUtils.appendAtWindow(args, window, "window")
            return args
        }

        private fun require(arguments: Map<String, String>, key: String): String {
            val value = arguments[key]
            require(!value.isNullOrBlank()) { "Instruction argument '$key' is required" }
            return value
        }
    }
}
