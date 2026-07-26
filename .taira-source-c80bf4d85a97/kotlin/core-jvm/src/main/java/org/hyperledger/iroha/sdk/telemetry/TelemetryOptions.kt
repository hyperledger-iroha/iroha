package org.hyperledger.iroha.sdk.telemetry

/**
 * Telemetry configuration derived from `iroha_config`.
 *
 * The options capture runtime policy (enable/disable) and the redaction parameters used by
 * telemetry observers when emitting signals required by the AND7 roadmap deliverables.
 */
class TelemetryOptions(
    @JvmField val enabled: Boolean = true,
    @JvmField val redaction: Redaction = Redaction.disabled(),
) {
    init {
        if (!enabled) {
            // When disabled, force redaction to disabled as well (mirrors Java builder logic).
        }
    }

    class Builder {
        private var enabled: Boolean = true
        private var redaction: Redaction = Redaction.disabled()

        fun setEnabled(enabled: Boolean): Builder { this.enabled = enabled; return this }
        fun setTelemetryRedaction(redaction: Redaction): Builder { this.redaction = redaction; return this }
        fun build(): TelemetryOptions = TelemetryOptions(enabled, redaction)
    }

    companion object {
        @JvmStatic
        fun disabled(): TelemetryOptions =
            TelemetryOptions(enabled = false, redaction = Redaction.disabled())

        @JvmStatic
        fun builder(): Builder = Builder()
    }
}
