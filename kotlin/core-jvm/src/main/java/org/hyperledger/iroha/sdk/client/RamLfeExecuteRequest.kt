package org.hyperledger.iroha.sdk.client

/** Typed request wrapper for RAM-LFE execute flows. */
class RamLfeExecuteRequest private constructor(
    @JvmField val encryptedInputHex: String,
) {
    companion object {
        @JvmStatic
        fun encrypted(encryptedInputHex: String): RamLfeExecuteRequest =
            RamLfeExecuteRequest(HttpClientTransport.normalizeEvenLengthHex(encryptedInputHex, "encryptedInputHex"))
    }
}
