package org.hyperledger.iroha.sdk.telemetry

import java.util.Optional
import java.util.OptionalInt
import java.util.OptionalLong

/**
 * Immutable snapshot describing a single HTTP request/response pair observed by the client.
 */
class TelemetryRecord(
    @JvmField val authorityHash: String?,
    @JvmField val saltVersion: String?,
    @JvmField val route: String?,
    @JvmField val method: String?,
    private val latencyMillisValue: Long? = null,
    private val statusCodeValue: Int? = null,
    @JvmField val errorKind: String? = null,
) {

    /** Returns a new record that copies this snapshot and adds outcome metadata. */
    fun withOutcome(
        latencyMillis: Long?,
        statusCode: Int?,
        errorKind: String?,
    ): TelemetryRecord {
        val normalisedLatency = latencyMillis?.let { maxOf(0L, it) }
        return TelemetryRecord(
            authorityHash, saltVersion, route, method,
            normalisedLatency, statusCode, errorKind,
        )
    }

    /** Returns the observed latency in milliseconds when available. */
    fun latencyMillis(): OptionalLong =
        if (latencyMillisValue == null) OptionalLong.empty()
        else OptionalLong.of(latencyMillisValue)

    /** Returns the HTTP status code (or `OptionalInt.empty()` when unknown). */
    fun statusCode(): OptionalInt =
        if (statusCodeValue == null) OptionalInt.empty()
        else OptionalInt.of(statusCodeValue)

    /** Returns the error classification when the request failed. */
    fun errorKind(): Optional<String> = Optional.ofNullable(errorKind)

    override fun toString(): String =
        "TelemetryRecord{" +
            "authorityHash='$authorityHash'" +
            ", saltVersion='$saltVersion'" +
            ", route='$route'" +
            ", method='$method'" +
            ", latencyMillis=$latencyMillisValue" +
            ", statusCode=$statusCodeValue" +
            ", errorKind='$errorKind'" +
            "}"

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is TelemetryRecord) return false
        return authorityHash == other.authorityHash
            && saltVersion == other.saltVersion
            && route == other.route
            && method == other.method
            && latencyMillisValue == other.latencyMillisValue
            && statusCodeValue == other.statusCodeValue
            && errorKind == other.errorKind
    }

    override fun hashCode(): Int {
        var result = authorityHash.hashCode()
        result = 31 * result + saltVersion.hashCode()
        result = 31 * result + route.hashCode()
        result = 31 * result + method.hashCode()
        result = 31 * result + latencyMillisValue.hashCode()
        result = 31 * result + statusCodeValue.hashCode()
        result = 31 * result + errorKind.hashCode()
        return result
    }
}
