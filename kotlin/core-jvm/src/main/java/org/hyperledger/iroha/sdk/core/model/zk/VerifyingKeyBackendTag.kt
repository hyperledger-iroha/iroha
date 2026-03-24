package org.hyperledger.iroha.sdk.core.model.zk

/**
 * Backend identifiers for verifying key records.
 *
 * Matches the Norito enums exposed by `iroha_data_model::zk::BackendTag`.
 */
enum class VerifyingKeyBackendTag(@JvmField val noritoValue: String) {
    HALO2_IPA_PASTA("halo2-ipa-pasta"),
    HALO2_BN254("halo2-bn254"),
    GROTH16("groth16"),
    STARK("stark"),
    UNSUPPORTED("unsupported");

    companion object {
        /** Parses a Norito backend string into the corresponding enum value. */
        @JvmStatic
        fun parse(value: String): VerifyingKeyBackendTag {
            val normalized = value.trim().lowercase()
            for (tag in entries) {
                if (tag.noritoValue == normalized) {
                    return tag
                }
            }
            throw IllegalArgumentException("unsupported backend tag: $value")
        }
    }
}
