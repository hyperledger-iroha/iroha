package org.hyperledger.iroha.sdk.core.model.instructions

import java.util.TreeMap
import org.hyperledger.iroha.sdk.multisig.MultisigSeedHelper
import org.hyperledger.iroha.sdk.multisig.MultisigSpec

private const val ACTION = "MultisigRegister"
private const val SIGNATORIES_PREFIX = "spec.signatories."

/** Typed representation of the `MultisigRegister` custom instruction. */
class MultisigRegisterInstruction(
    @JvmField val accountId: String,
    @JvmField val spec: MultisigSpec,
    arguments: Map<String, String>? = null,
) : InstructionTemplate {

    private val _arguments: Map<String, String> = arguments?.toMap() ?: canonicalArguments()

    override val kind: InstructionKind = InstructionKind.CUSTOM

    override val arguments: Map<String, String> get() = _arguments

    init {
        require(accountId.isNotBlank()) { "accountId must not be blank" }
        val signatoryDomain = signatoryDomain(spec)
        val controllerDomain = extractDomain(accountId, "accountId")
        require(controllerDomain == signatoryDomain) {
            "multisig account '$accountId' must belong to domain '$signatoryDomain'"
        }
        require(!MultisigSeedHelper.isDeterministicDerivedControllerId(accountId, spec)) {
            "multisig account uses deterministically derived controller id; register with a random controller"
        }
    }

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is MultisigRegisterInstruction) return false
        return accountId == other.accountId && spec == other.spec
    }

    override fun hashCode(): Int = listOf(accountId, spec).hashCode()

    private fun canonicalArguments(): Map<String, String> = buildMap {
        put("action", ACTION)
        put("account", accountId)
        put("spec.quorum", spec.quorum.toString())
        put("spec.transaction_ttl_ms", spec.transactionTtlMs.toString())
        val sortedSignatories = TreeMap(spec.signatories)
        sortedSignatories.forEach { (account, weight) ->
            put("$SIGNATORIES_PREFIX$account", weight.toString())
        }
    }

    companion object {
        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): MultisigRegisterInstruction {
            val parsedSpec = parseSpec(arguments)
            return MultisigRegisterInstruction(
                accountId = requireArgument(arguments, "account"),
                spec = parsedSpec,
                arguments = LinkedHashMap(arguments),
            )
        }

        private fun parseSpec(arguments: Map<String, String>): MultisigSpec {
            val signatories = buildMap<String, Int> {
                for ((key, _) in arguments) {
                    if (key.startsWith(SIGNATORIES_PREFIX)) {
                        val accountId = key.substring(SIGNATORIES_PREFIX.length)
                        put(accountId, requireInt(arguments, key))
                    }
                }
            }
            require(signatories.isNotEmpty()) {
                "At least one signatory is required via '${SIGNATORIES_PREFIX}<account_id>'"
            }
            try {
                return MultisigSpec(
                    signatories = signatories,
                    quorum = requireInt(arguments, "spec.quorum"),
                    transactionTtlMs = requireLong(arguments, "spec.transaction_ttl_ms"),
                )
            } catch (ex: IllegalStateException) {
                throw IllegalArgumentException("Invalid multisig spec: ${ex.message}", ex)
            }
        }

        private fun requireArgument(arguments: Map<String, String>, key: String): String {
            val value = arguments[key]
            require(!value.isNullOrBlank()) { "Instruction argument '$key' is required" }
            return value
        }

        private fun requireInt(arguments: Map<String, String>, key: String): Int {
            val value = requireArgument(arguments, key)
            try {
                return value.toInt()
            } catch (ex: NumberFormatException) {
                throw IllegalArgumentException(
                    "Instruction argument '$key' must be an integer: $value", ex
                )
            }
        }

        private fun requireLong(arguments: Map<String, String>, key: String): Long {
            val value = requireArgument(arguments, key)
            try {
                return value.toLong()
            } catch (ex: NumberFormatException) {
                throw IllegalArgumentException(
                    "Instruction argument '$key' must be a long: $value", ex
                )
            }
        }

        private fun signatoryDomain(spec: MultisigSpec): String {
            var domain: String? = null
            for (accountId in spec.signatories.keys) {
                val signatoryDomain = extractDomain(accountId, "signatory")
                if (domain == null) {
                    domain = signatoryDomain
                    continue
                }
                check(domain == signatoryDomain) {
                    "multisig signatory '$accountId' must belong to domain '$domain'"
                }
            }
            return checkNotNull(domain) { "multisig spec must include at least one signatory domain" }
        }

        private fun extractDomain(accountId: String, field: String): String {
            check(accountId.isNotBlank()) { "$field must not be blank" }
            val trimmed = accountId.trim()
            val atIndex = trimmed.lastIndexOf('@')
            check(atIndex > 0 && atIndex != trimmed.length - 1) { "$field must include a domain" }
            val domain = trimmed.substring(atIndex + 1).trim()
            check(domain.isNotBlank()) { "$field must include a domain" }
            return domain
        }
    }
}
