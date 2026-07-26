package org.hyperledger.iroha.sdk.connect

/** Decoded payload for a REJECT control frame. */
class RejectControl internal constructor(
    @JvmField val code: Int,
    @JvmField val codeId: String,
    @JvmField val reason: String,
)
