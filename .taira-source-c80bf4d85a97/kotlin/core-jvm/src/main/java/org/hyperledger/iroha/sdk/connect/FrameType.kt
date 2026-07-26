package org.hyperledger.iroha.sdk.connect

/** Frame type discriminator for decoded Connect frames. */
enum class FrameType {
    OPEN,
    REJECT,
    CLOSE,
    CIPHERTEXT,
    OTHER_CONTROL,
}
