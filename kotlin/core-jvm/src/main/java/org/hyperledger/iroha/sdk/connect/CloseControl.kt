package org.hyperledger.iroha.sdk.connect

/** Decoded payload for a CLOSE control frame. */
class CloseControl internal constructor(
    @JvmField val role: ConnectRole,
    @JvmField val code: Int,
    @JvmField val reason: String,
    @JvmField val retryable: Boolean,
)
