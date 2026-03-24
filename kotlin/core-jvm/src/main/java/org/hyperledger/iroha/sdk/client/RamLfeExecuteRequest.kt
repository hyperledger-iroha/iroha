package org.hyperledger.iroha.sdk.client

/** Typed request wrapper for RAM-LFE execute flows. */
class RamLfeExecuteRequest private constructor(
    @JvmField val inputHex: String?,
    @JvmField val encryptedInputHex: String?,
) {
    companion object {
        @JvmStatic
        fun plaintext(inputHex: String): RamLfeExecuteRequest =
            RamLfeExecuteRequest(
                HttpClientTransport.normalizeEvenLengthHex(inputHex, "inputHex"),
                null,
            )

        @JvmStatic
        fun encrypted(encryptedInputHex: String): RamLfeExecuteRequest =
            RamLfeExecuteRequest(
                null,
                HttpClientTransport.normalizeEvenLengthHex(encryptedInputHex, "encryptedInputHex"),
            )
    }
}
