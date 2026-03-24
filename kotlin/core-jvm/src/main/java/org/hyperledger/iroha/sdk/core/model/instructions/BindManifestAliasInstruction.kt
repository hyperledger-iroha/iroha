package org.hyperledger.iroha.sdk.core.model.instructions

private const val BIND_ALIAS_ACTION = "BindManifestAlias"

/**
 * Typed representation of the `BindManifestAlias` instruction (SoraFS alias lifecycle).
 */
class BindManifestAliasInstruction private constructor(
    val digestHex: String,
    val aliasBinding: RegisterPinManifestInstruction.AliasBinding,
    val boundEpoch: Long,
    val expiryEpoch: Long,
    private val _arguments: Map<String, String>,
) : InstructionTemplate {

    override val kind: InstructionKind get() = InstructionKind.CUSTOM

    override val arguments: Map<String, String> get() = _arguments

    constructor(
        digestHex: String,
        aliasBinding: RegisterPinManifestInstruction.AliasBinding,
        boundEpoch: Long,
        expiryEpoch: Long,
    ) : this(
        digestHex = digestHex.also {
            require(it.isNotBlank()) { "digestHex must not be blank" }
        },
        aliasBinding = aliasBinding,
        boundEpoch = boundEpoch.also {
            require(it >= 0) { "boundEpoch must be non-negative" }
        },
        expiryEpoch = expiryEpoch.also {
            require(it >= 0) { "expiryEpoch must be non-negative" }
        },
        _arguments = buildCanonicalArguments(digestHex, aliasBinding, boundEpoch, expiryEpoch),
    )

    constructor(
        digestHex: String,
        aliasName: String,
        aliasNamespace: String,
        proofHex: String,
        boundEpoch: Long,
        expiryEpoch: Long,
    ) : this(
        digestHex = digestHex,
        aliasBinding = RegisterPinManifestInstruction.AliasBinding.builder()
            .setName(aliasName)
            .setNamespace(aliasNamespace)
            .setProofHex(proofHex)
            .build(),
        boundEpoch = boundEpoch,
        expiryEpoch = expiryEpoch,
    )

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is BindManifestAliasInstruction) return false
        return boundEpoch == other.boundEpoch
            && expiryEpoch == other.expiryEpoch
            && digestHex == other.digestHex
            && aliasBinding == other.aliasBinding
    }

    override fun hashCode(): Int {
        var result = digestHex.hashCode()
        result = 31 * result + aliasBinding.hashCode()
        result = 31 * result + boundEpoch.hashCode()
        result = 31 * result + expiryEpoch.hashCode()
        return result
    }

    companion object {
        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): BindManifestAliasInstruction {
            val digestHex = require(arguments, "digest_hex")
            val boundEpoch = requireLong(arguments, "bound_epoch")
            val expiryEpoch = requireLong(arguments, "expiry_epoch")
            val aliasBinding = RegisterPinManifestInstruction.AliasBinding.fromArguments(arguments, true)!!
            return BindManifestAliasInstruction(
                digestHex = digestHex,
                aliasBinding = aliasBinding,
                boundEpoch = boundEpoch,
                expiryEpoch = expiryEpoch,
                _arguments = LinkedHashMap(arguments),
            )
        }

        private fun buildCanonicalArguments(
            digestHex: String,
            aliasBinding: RegisterPinManifestInstruction.AliasBinding,
            boundEpoch: Long,
            expiryEpoch: Long,
        ): Map<String, String> {
            val args = linkedMapOf(
                "action" to BIND_ALIAS_ACTION,
                "digest_hex" to digestHex,
                "bound_epoch" to boundEpoch.toString(),
                "expiry_epoch" to expiryEpoch.toString(),
            )
            aliasBinding.appendArguments(args)
            return args
        }

        private fun require(arguments: Map<String, String>, key: String): String {
            val value = arguments[key]
            require(!value.isNullOrBlank()) { "Instruction argument '$key' is required" }
            return value
        }

        private fun requireLong(arguments: Map<String, String>, key: String): Long {
            val value = require(arguments, key)
            try {
                return value.toLong()
            } catch (ex: NumberFormatException) {
                throw IllegalArgumentException(
                    "Instruction argument '$key' must be a number: $value", ex,
                )
            }
        }
    }
}
