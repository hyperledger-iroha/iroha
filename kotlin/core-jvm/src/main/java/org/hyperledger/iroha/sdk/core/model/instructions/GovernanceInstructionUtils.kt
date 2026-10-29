package org.hyperledger.iroha.sdk.core.model.instructions

/** Helper utilities shared across governance instruction builders. */
object GovernanceInstructionUtils {

    private const val GOVERNANCE_SELECTOR_V1_MAX_LENGTH = 128
    private const val GOVERNANCE_SELECTOR_V1_PATTERN =
        "^[A-Za-z0-9_~-][A-Za-z0-9._~-]{0,127}$"

    /** Require one canonical first-release governance selector without normalizing it. */
    @JvmStatic
    fun requireGovernanceSelectorV1(value: String, fieldName: String): String {
        require(
            value.length in 1..GOVERNANCE_SELECTOR_V1_MAX_LENGTH &&
                value[0] != '.' &&
                value.all(::isGovernanceSelectorUnreservedAscii),
        ) {
            "$fieldName must match $GOVERNANCE_SELECTOR_V1_PATTERN"
        }
        return value
    }

    private fun isGovernanceSelectorUnreservedAscii(character: Char): Boolean =
        character in 'A'..'Z' ||
            character in 'a'..'z' ||
            character in '0'..'9' ||
            character == '-' ||
            character == '.' ||
            character == '_' ||
            character == '~'
}
