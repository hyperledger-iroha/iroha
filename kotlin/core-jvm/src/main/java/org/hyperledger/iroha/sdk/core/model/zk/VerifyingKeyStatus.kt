package org.hyperledger.iroha.sdk.core.model.zk

/**
 * Lifecycle status for verifying keys as tracked in the registry.
 *
 * Matches `iroha_data_model::confidential::ConfidentialStatus`.
 */
enum class VerifyingKeyStatus(@JvmField val wireName: String) {
    PROPOSED("Proposed"),
    ACTIVE("Active"),
    WITHDRAWN("Withdrawn");

    companion object {
        /** Parses the wire representation into an enum value. */
        @JvmStatic
        fun parse(value: String): VerifyingKeyStatus {
            val normalized = value.trim()
            for (status in entries) {
                if (status.wireName.equals(normalized, ignoreCase = true)) {
                    return status
                }
            }
            throw IllegalArgumentException("unsupported verifying key status: $value")
        }
    }
}
