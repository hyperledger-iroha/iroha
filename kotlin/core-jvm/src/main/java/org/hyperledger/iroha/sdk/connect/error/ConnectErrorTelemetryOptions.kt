package org.hyperledger.iroha.sdk.connect.error

/** Optional overrides when rendering telemetry attributes. */
data class ConnectErrorTelemetryOptions(
    @JvmField val fatal: Boolean?,
    @JvmField val httpStatus: Int?,
    @JvmField val underlying: String?,
) {
    companion object {
        @JvmStatic
        fun empty(): ConnectErrorTelemetryOptions =
            ConnectErrorTelemetryOptions(null, null, null)
    }

    fun withFatal(value: Boolean?): ConnectErrorTelemetryOptions {
        if ((fatal == null && value == null) || (fatal != null && fatal == value)) {
            return this
        }
        return ConnectErrorTelemetryOptions(value, httpStatus, underlying)
    }
}
