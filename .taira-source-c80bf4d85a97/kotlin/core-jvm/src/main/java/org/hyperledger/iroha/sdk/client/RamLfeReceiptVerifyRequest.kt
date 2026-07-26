package org.hyperledger.iroha.sdk.client

/** Typed request wrapper for RAM-LFE receipt verification. */
class RamLfeReceiptVerifyRequest(
    receipt: Map<String, Any>,
    @JvmField val outputHex: String?,
) {
    @JvmField
    val receipt: Map<String, Any> = receipt.toMap()
}
