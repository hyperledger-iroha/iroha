package org.hyperledger.iroha.sdk.telemetry

/** Supplies metadata describing a captured crash. */
fun interface MetadataProvider {
    fun capture(thread: Thread, error: Throwable): CrashMetadata?
}
