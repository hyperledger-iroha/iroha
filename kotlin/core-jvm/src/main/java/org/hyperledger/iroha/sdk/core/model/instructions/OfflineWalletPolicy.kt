package org.hyperledger.iroha.sdk.core.model.instructions

/** Certificate policy bounds for an offline wallet. */
class OfflineWalletPolicy(
    @JvmField val maxBalance: String,
    @JvmField val maxTxValue: String,
    @JvmField val expiresAtMs: Long,
) {
    init {
        require(maxBalance.isNotBlank()) { "maxBalance must not be blank" }
        require(maxTxValue.isNotBlank()) { "maxTxValue must not be blank" }
        require(expiresAtMs > 0L) { "expiresAtMs must be positive" }
    }

    fun appendArguments(target: MutableMap<String, String>) {
        target["certificate.policy.max_balance"] = maxBalance
        target["certificate.policy.max_tx_value"] = maxTxValue
        target["certificate.policy.expires_at_ms"] = expiresAtMs.toString()
    }

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is OfflineWalletPolicy) return false
        return maxBalance == other.maxBalance
            && maxTxValue == other.maxTxValue
            && expiresAtMs == other.expiresAtMs
    }

    override fun hashCode(): Int = listOf(maxBalance, maxTxValue, expiresAtMs).hashCode()

    companion object {
        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): OfflineWalletPolicy {
            return OfflineWalletPolicy(
                maxBalance = requireArgument(arguments, "certificate.policy.max_balance"),
                maxTxValue = requireArgument(arguments, "certificate.policy.max_tx_value"),
                expiresAtMs = requireLong(arguments, "certificate.policy.expires_at_ms"),
            )
        }

        private fun requireArgument(arguments: Map<String, String>, key: String): String {
            val value = arguments[key]
            require(!value.isNullOrBlank()) { "Instruction argument '$key' is required" }
            return value
        }

        private fun requireLong(arguments: Map<String, String>, key: String): Long {
            val raw = requireArgument(arguments, key)
            return raw.toLongOrNull()
                ?: throw IllegalArgumentException("Instruction argument '$key' must be numeric: $raw")
        }
    }
}
