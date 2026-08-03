package org.hyperledger.iroha.sdk.core.model.instructions

/** Helper utilities shared across governance instruction builders. */
object GovernanceInstructionUtils {

    private val HEX_PATTERN = Regex("^[0-9a-fA-F]+$")
    private val LOWERCASE_HEX_PATTERN = Regex("^[0-9a-f]+$")

    private const val GOVERNANCE_SELECTOR_V1_MAX_LENGTH = 128
    private const val GOVERNANCE_SELECTOR_V1_PATTERN =
        "^[A-Za-z0-9_~-][A-Za-z0-9._~-]{0,127}$"

    /** Inclusive enactment window expressed in block heights. */
    class AtWindow(@JvmField val lower: Long, @JvmField val upper: Long) {
        init {
            require(lower >= 0 && upper >= lower) {
                "window bounds must satisfy 0 <= lower <= upper"
            }
        }
    }

    /** Voting mode applied to referendums spawned by a proposal. */
    enum class VotingMode(@JvmField val wireValue: String) {
        ZK("Zk"),
        PLAIN("Plain");

        companion object {
            @JvmStatic
            fun parse(raw: String): VotingMode {
                require(raw.isNotBlank()) { "mode must not be blank" }
                return when (raw) {
                    "Zk" -> ZK
                    "Plain" -> PLAIN
                    else -> throw IllegalArgumentException("Unknown voting mode: $raw")
                }
            }
        }
    }

    @JvmStatic
    fun appendAtWindow(arguments: MutableMap<String, String>, window: AtWindow, prefix: String) {
        arguments["$prefix.lower"] = window.lower.toString()
        arguments["$prefix.upper"] = window.upper.toString()
    }

    @JvmStatic
    fun parseAtWindow(
        arguments: Map<String, String>,
        prefix: String,
        displayName: String,
    ): AtWindow {
        val lowerRaw = arguments["$prefix.lower"]
        val upperRaw = arguments["$prefix.upper"]
        require(lowerRaw != null && upperRaw != null) {
            "$displayName must include lower and upper bounds"
        }
        try {
            return AtWindow(lowerRaw.toLong(), upperRaw.toLong())
        } catch (ex: NumberFormatException) {
            throw IllegalArgumentException(
                "Window bounds must be numeric for $displayName: lower=$lowerRaw, upper=$upperRaw",
                ex,
            )
        }
    }

    @JvmStatic
    fun requireHex(value: String?, fieldName: String, expectedBytes: Int): String {
        require(!value.isNullOrBlank()) { "$fieldName must not be blank" }
        val normalized = if (value.startsWith("0x")) value.substring(2) else value
        require(HEX_PATTERN.matches(normalized)) { "$fieldName must be hexadecimal: $value" }
        if (expectedBytes > 0) {
            require(normalized.length == expectedBytes * 2) {
                "$fieldName must be ${expectedBytes * 2} hex chars, found ${normalized.length}"
            }
        }
        return normalized.lowercase()
    }

    /** Require an exact lowercase hexadecimal value without compatibility normalization. */
    @JvmStatic
    fun requireExactLowercaseHex(value: String?, fieldName: String, expectedBytes: Int): String {
        require(expectedBytes > 0) { "expectedBytes must be positive" }
        require(!value.isNullOrBlank()) { "$fieldName must not be blank" }
        require(value.length == expectedBytes * 2 && LOWERCASE_HEX_PATTERN.matches(value)) {
            "$fieldName must be exactly ${expectedBytes * 2} lowercase hex chars"
        }
        return value
    }

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
