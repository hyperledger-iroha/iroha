package org.hyperledger.iroha.sdk.telemetry

/** Value object describing the recorded crash. */
class CrashMetadata(
    @JvmField val crashId: String,
    @JvmField val signal: String,
    @JvmField val processState: String,
    @JvmField val hasNativeTrace: Boolean,
    @JvmField val watchdogBucket: String,
)
