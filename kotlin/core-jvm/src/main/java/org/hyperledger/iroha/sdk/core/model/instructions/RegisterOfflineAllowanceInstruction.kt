package org.hyperledger.iroha.sdk.core.model.instructions

private const val ACTION = "RegisterOfflineAllowance"

/** Typed representation of the `RegisterOfflineAllowance` instruction. */
class RegisterOfflineAllowanceInstruction(
    @JvmField val certificate: OfflineWalletCertificate,
    arguments: Map<String, String>? = null,
) : InstructionTemplate {

    private val _arguments: Map<String, String> = arguments?.toMap() ?: canonicalArguments()

    override val kind: InstructionKind = InstructionKind.CUSTOM

    override val arguments: Map<String, String> get() = _arguments

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is RegisterOfflineAllowanceInstruction) return false
        return certificate == other.certificate
    }

    override fun hashCode(): Int = certificate.hashCode()

    private fun canonicalArguments(): Map<String, String> = buildMap {
        put("action", ACTION)
        certificate.appendArguments(this)
    }

    companion object {
        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): RegisterOfflineAllowanceInstruction {
            return RegisterOfflineAllowanceInstruction(
                certificate = OfflineWalletCertificate.fromArguments(arguments),
                arguments = arguments,
            )
        }
    }
}
