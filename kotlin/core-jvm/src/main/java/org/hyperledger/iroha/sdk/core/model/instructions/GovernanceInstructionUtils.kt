package org.hyperledger.iroha.sdk.core.model.instructions

/** Helper utilities shared across governance instruction builders. */
object GovernanceInstructionUtils {

    private val HEX_PATTERN = Regex("^[0-9a-fA-F]+$")

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
                return when (raw.trim().lowercase()) {
                    "zk" -> ZK
                    "plain" -> PLAIN
                    else -> throw IllegalArgumentException("Unknown voting mode: $raw")
                }
            }
        }
    }

    /** Derivation kind recorded for persisted councils. */
    enum class CouncilDerivationKind(@JvmField val wireValue: String) {
        VRF("Vrf"),
        FALLBACK("Fallback");

        companion object {
            @JvmStatic
            fun parse(raw: String): CouncilDerivationKind {
                require(raw.isNotBlank()) { "derived_by must not be blank" }
                return when (raw.trim().lowercase()) {
                    "vrf" -> VRF
                    "fallback" -> FALLBACK
                    else -> throw IllegalArgumentException("Unknown council derivation: $raw")
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
}
