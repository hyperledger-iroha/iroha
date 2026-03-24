package org.hyperledger.iroha.sdk.core.model.instructions

private const val ACTION = "ProposeDeployContract"

/**
 * Typed representation of a `ProposeDeployContract` instruction.
 *
 * Captures the governance namespace, contract identifiers, deterministic code/ABI hashes, and
 * optional enactment window + voting mode overrides.
 */
class ProposeDeployContractInstruction(
    @JvmField val namespace: String,
    @JvmField val contractId: String,
    codeHashHex: String,
    abiHashHex: String,
    @JvmField val abiVersion: String,
    @JvmField val window: GovernanceInstructionUtils.AtWindow? = null,
    @JvmField val votingMode: GovernanceInstructionUtils.VotingMode? = null,
) : InstructionTemplate {

    @JvmField val codeHashHex: String = GovernanceInstructionUtils.requireHex(codeHashHex, "codeHashHex", 32)
    @JvmField val abiHashHex: String = GovernanceInstructionUtils.requireHex(abiHashHex, "abiHashHex", 32)

    init {
        require(namespace.isNotBlank()) { "namespace must not be blank" }
        require(contractId.isNotBlank()) { "contractId must not be blank" }
        require(abiVersion.isNotBlank()) { "abiVersion must not be blank" }
    }

    override val kind: InstructionKind = InstructionKind.CUSTOM

    override val arguments: Map<String, String> by lazy { canonicalArguments() }

    private fun canonicalArguments(): Map<String, String> {
        val args = linkedMapOf<String, String>()
        args["action"] = ACTION
        args["namespace"] = namespace
        args["contract_id"] = contractId
        args["code_hash_hex"] = codeHashHex
        args["abi_hash_hex"] = abiHashHex
        args["abi_version"] = abiVersion
        if (window != null) {
            GovernanceInstructionUtils.appendAtWindow(args, window, "window")
        }
        if (votingMode != null) {
            args["mode"] = votingMode.wireValue
        }
        return args
    }

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is ProposeDeployContractInstruction) return false
        return namespace == other.namespace
            && contractId == other.contractId
            && codeHashHex == other.codeHashHex
            && abiHashHex == other.abiHashHex
            && abiVersion == other.abiVersion
            && window == other.window
            && votingMode == other.votingMode
    }

    override fun hashCode(): Int {
        var result = namespace.hashCode()
        result = 31 * result + contractId.hashCode()
        result = 31 * result + codeHashHex.hashCode()
        result = 31 * result + abiHashHex.hashCode()
        result = 31 * result + abiVersion.hashCode()
        result = 31 * result + (window?.hashCode() ?: 0)
        result = 31 * result + (votingMode?.hashCode() ?: 0)
        return result
    }

    companion object {
        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): ProposeDeployContractInstruction {
            val namespace = requireArg(arguments, "namespace")
            val contractId = requireArg(arguments, "contract_id")
            val codeHashHex = requireArg(arguments, "code_hash_hex")
            val abiHashHex = requireArg(arguments, "abi_hash_hex")
            val abiVersion = requireArg(arguments, "abi_version")

            val votingMode = arguments["mode"]?.let {
                GovernanceInstructionUtils.VotingMode.parse(it)
            }
            val window = if (arguments.containsKey("window.lower") || arguments.containsKey("window.upper")) {
                GovernanceInstructionUtils.parseAtWindow(arguments, "window", "window override")
            } else {
                null
            }

            return ProposeDeployContractInstruction(
                namespace = namespace,
                contractId = contractId,
                codeHashHex = codeHashHex,
                abiHashHex = abiHashHex,
                abiVersion = abiVersion,
                window = window,
                votingMode = votingMode,
            )
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
