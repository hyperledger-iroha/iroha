package org.hyperledger.iroha.sdk.core.model.instructions

/** Matches the discriminants exposed by `iroha_data_model::isi::InstructionType`.  */
enum class InstructionKind(@JvmField val displayName: String, @JvmField val discriminant: Int) {
    SET_PARAMETER("SetParameter", 0),
    SET_KEY_VALUE("SetKeyValue", 1),
    REMOVE_KEY_VALUE("RemoveKeyValue", 2),
    REGISTER("Register", 3),
    UNREGISTER("Unregister", 4),
    MINT("Mint", 5),
    BURN("Burn", 6),
    TRANSFER("Transfer", 7),
    GRANT("Grant", 8),
    REVOKE("Revoke", 9),
    UPGRADE("Upgrade", 10),
    EXECUTE_TRIGGER("ExecuteTrigger", 11),
    LOG("Log", 12),
    CUSTOM("Custom", 13);

    companion object {
        @JvmStatic
        fun fromDisplayName(name: String): InstructionKind {
            val normalized = name.lowercase()
            for (kind in entries) {
                if (kind.displayName.lowercase() == normalized) {
                    return kind
                }
            }
            throw IllegalArgumentException("Unknown instruction kind: $name")
        }

        @JvmStatic
        fun fromDiscriminant(value: Long): InstructionKind {
            for (kind in entries) {
                if (kind.discriminant.toLong() == value) {
                    return kind
                }
            }
            throw IllegalArgumentException("Unknown instruction discriminant: $value")
        }
    }
}
