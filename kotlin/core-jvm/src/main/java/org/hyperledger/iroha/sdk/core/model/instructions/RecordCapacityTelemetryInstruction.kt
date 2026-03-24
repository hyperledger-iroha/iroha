package org.hyperledger.iroha.sdk.core.model.instructions

private const val ACTION = "RecordCapacityTelemetry"

/** Typed representation of a `RecordCapacityTelemetry` instruction. */
class RecordCapacityTelemetryInstruction(
    @JvmField val providerIdHex: String,
    @JvmField val windowStartEpoch: Long,
    @JvmField val windowEndEpoch: Long,
    @JvmField val declaredGib: Long,
    @JvmField val effectiveGib: Long,
    @JvmField val utilisedGib: Long,
    @JvmField val ordersIssued: Long,
    @JvmField val ordersCompleted: Long,
    @JvmField val uptimeBps: Int,
    @JvmField val porSuccessBps: Int,
    @JvmField val egressBytes: Long = 0L,
    @JvmField val pdpChallenges: Int = 0,
    @JvmField val pdpFailures: Int = 0,
    @JvmField val potrWindows: Int = 0,
    @JvmField val potrBreaches: Int = 0,
    @JvmField val nonce: Long = 0L,
) : InstructionTemplate {

    init {
        require(providerIdHex.isNotBlank()) { "providerIdHex must not be blank" }
        require(windowStartEpoch >= 0) { "windowStartEpoch must be non-negative" }
        require(windowEndEpoch >= 0) { "windowEndEpoch must be non-negative" }
        require(windowEndEpoch >= windowStartEpoch) { "windowEndEpoch must be >= windowStartEpoch" }
        require(declaredGib >= 0) { "declaredGib must be non-negative" }
        require(effectiveGib >= 0) { "effectiveGib must be non-negative" }
        require(utilisedGib >= 0) { "utilisedGib must be non-negative" }
        require(ordersIssued >= 0) { "ordersIssued must be non-negative" }
        require(ordersCompleted >= 0) { "ordersCompleted must be non-negative" }
        require(uptimeBps >= 0) { "uptimeBps must be non-negative" }
        require(porSuccessBps >= 0) { "porSuccessBps must be non-negative" }
        require(egressBytes >= 0) { "egressBytes must be non-negative" }
        require(pdpChallenges >= 0) { "pdpChallenges must be non-negative" }
        require(pdpFailures >= 0) { "pdpFailures must be non-negative" }
        require(potrWindows >= 0) { "potrWindows must be non-negative" }
        require(potrBreaches >= 0) { "potrBreaches must be non-negative" }
        require(nonce >= 0) { "nonce must be non-negative" }
    }

    override val kind: InstructionKind = InstructionKind.CUSTOM

    override val arguments: Map<String, String> by lazy { canonicalArguments() }

    private fun canonicalArguments(): Map<String, String> {
        val args = linkedMapOf<String, String>()
        args["action"] = ACTION
        args["provider_id_hex"] = providerIdHex
        args["window_start_epoch"] = windowStartEpoch.toString()
        args["window_end_epoch"] = windowEndEpoch.toString()
        args["declared_gib"] = declaredGib.toString()
        args["effective_gib"] = effectiveGib.toString()
        args["utilised_gib"] = utilisedGib.toString()
        args["orders_issued"] = ordersIssued.toString()
        args["orders_completed"] = ordersCompleted.toString()
        args["uptime_bps"] = uptimeBps.toString()
        args["por_success_bps"] = porSuccessBps.toString()
        if (egressBytes != 0L) args["egress_bytes"] = egressBytes.toString()
        if (pdpChallenges != 0) args["pdp_challenges"] = pdpChallenges.toString()
        if (pdpFailures != 0) args["pdp_failures"] = pdpFailures.toString()
        if (potrWindows != 0) args["potr_windows"] = potrWindows.toString()
        if (potrBreaches != 0) args["potr_breaches"] = potrBreaches.toString()
        if (nonce != 0L) args["nonce"] = nonce.toString()
        return args
    }

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is RecordCapacityTelemetryInstruction) return false
        return providerIdHex == other.providerIdHex
            && windowStartEpoch == other.windowStartEpoch
            && windowEndEpoch == other.windowEndEpoch
            && declaredGib == other.declaredGib
            && effectiveGib == other.effectiveGib
            && utilisedGib == other.utilisedGib
            && ordersIssued == other.ordersIssued
            && ordersCompleted == other.ordersCompleted
            && uptimeBps == other.uptimeBps
            && porSuccessBps == other.porSuccessBps
            && egressBytes == other.egressBytes
            && pdpChallenges == other.pdpChallenges
            && pdpFailures == other.pdpFailures
            && potrWindows == other.potrWindows
            && potrBreaches == other.potrBreaches
            && nonce == other.nonce
    }

    override fun hashCode(): Int {
        var result = providerIdHex.hashCode()
        result = 31 * result + windowStartEpoch.hashCode()
        result = 31 * result + windowEndEpoch.hashCode()
        result = 31 * result + declaredGib.hashCode()
        result = 31 * result + effectiveGib.hashCode()
        result = 31 * result + utilisedGib.hashCode()
        result = 31 * result + ordersIssued.hashCode()
        result = 31 * result + ordersCompleted.hashCode()
        result = 31 * result + uptimeBps
        result = 31 * result + porSuccessBps
        result = 31 * result + egressBytes.hashCode()
        result = 31 * result + pdpChallenges
        result = 31 * result + pdpFailures
        result = 31 * result + potrWindows
        result = 31 * result + potrBreaches
        result = 31 * result + nonce.hashCode()
        return result
    }

    companion object {
        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): RecordCapacityTelemetryInstruction {
            return RecordCapacityTelemetryInstruction(
                providerIdHex = requireArg(arguments, "provider_id_hex"),
                windowStartEpoch = requireLong(arguments, "window_start_epoch"),
                windowEndEpoch = requireLong(arguments, "window_end_epoch"),
                declaredGib = requireLong(arguments, "declared_gib"),
                effectiveGib = requireLong(arguments, "effective_gib"),
                utilisedGib = requireLong(arguments, "utilised_gib"),
                ordersIssued = requireLong(arguments, "orders_issued"),
                ordersCompleted = requireLong(arguments, "orders_completed"),
                uptimeBps = requireInt(arguments, "uptime_bps"),
                porSuccessBps = requireInt(arguments, "por_success_bps"),
                egressBytes = parseOptionalLong(arguments, "egress_bytes") ?: 0L,
                pdpChallenges = parseOptionalInt(arguments, "pdp_challenges") ?: 0,
                pdpFailures = parseOptionalInt(arguments, "pdp_failures") ?: 0,
                potrWindows = parseOptionalInt(arguments, "potr_windows") ?: 0,
                potrBreaches = parseOptionalInt(arguments, "potr_breaches") ?: 0,
                nonce = parseOptionalLong(arguments, "nonce") ?: 0L,
            )
        }

        private fun requireArg(arguments: Map<String, String>, key: String): String {
            val value = arguments[key]
            if (value.isNullOrBlank()) {
                throw IllegalArgumentException("Instruction argument '$key' is required")
            }
            return value
        }

        private fun requireLong(arguments: Map<String, String>, key: String): Long {
            val raw = requireArg(arguments, key)
            try {
                return raw.toLong()
            } catch (ex: NumberFormatException) {
                throw IllegalArgumentException("Instruction argument '$key' must be a number: $raw", ex)
            }
        }

        private fun requireInt(arguments: Map<String, String>, key: String): Int {
            val raw = requireArg(arguments, key)
            try {
                return raw.toInt()
            } catch (ex: NumberFormatException) {
                throw IllegalArgumentException("Instruction argument '$key' must be a number: $raw", ex)
            }
        }

        private fun parseOptionalLong(arguments: Map<String, String>, key: String): Long? {
            val value = arguments[key]
            if (value.isNullOrBlank()) return null
            try {
                return value.toLong()
            } catch (ex: NumberFormatException) {
                throw IllegalArgumentException("$key must be a number: $value", ex)
            }
        }

        private fun parseOptionalInt(arguments: Map<String, String>, key: String): Int? {
            val value = arguments[key]
            if (value.isNullOrBlank()) return null
            try {
                return value.toInt()
            } catch (ex: NumberFormatException) {
                throw IllegalArgumentException("$key must be a number: $value", ex)
            }
        }
    }
}
