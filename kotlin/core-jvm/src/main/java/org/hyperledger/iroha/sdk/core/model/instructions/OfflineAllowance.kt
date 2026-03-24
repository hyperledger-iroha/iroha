package org.hyperledger.iroha.sdk.core.model.instructions

/** Allowance commitment embedded in an offline wallet certificate. */
class OfflineAllowance(
    @JvmField val assetId: String,
    @JvmField val amount: String,
    @JvmField val commitmentHex: String,
) {
    init {
        require(assetId.isNotBlank()) { "assetId must not be blank" }
        require(amount.isNotBlank()) { "amount must not be blank" }
        require(commitmentHex.isNotBlank()) { "commitmentHex must not be blank" }
    }

    fun appendArguments(target: MutableMap<String, String>) {
        target["certificate.allowance.asset"] = assetId
        target["certificate.allowance.amount"] = amount
        target["certificate.allowance.commitment_hex"] = commitmentHex
    }

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is OfflineAllowance) return false
        return assetId == other.assetId
            && amount == other.amount
            && commitmentHex == other.commitmentHex
    }

    override fun hashCode(): Int = listOf(assetId, amount, commitmentHex).hashCode()

    companion object {
        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): OfflineAllowance {
            return OfflineAllowance(
                assetId = requireArgument(arguments, "certificate.allowance.asset"),
                amount = requireArgument(arguments, "certificate.allowance.amount"),
                commitmentHex = requireArgument(arguments, "certificate.allowance.commitment_hex"),
            )
        }

        private fun requireArgument(arguments: Map<String, String>, key: String): String {
            val value = arguments[key]
            require(!value.isNullOrBlank()) { "Instruction argument '$key' is required" }
            return value
        }
    }
}
