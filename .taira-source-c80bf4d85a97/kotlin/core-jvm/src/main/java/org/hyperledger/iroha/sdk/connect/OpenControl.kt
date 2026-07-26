package org.hyperledger.iroha.sdk.connect

/** Decoded payload for an OPEN control frame. */
class OpenControl internal constructor(
    appPublicKey: ByteArray,
    @JvmField val chainId: String,
) {
    private val _appPublicKey: ByteArray = appPublicKey.copyOf()

    fun appPublicKey(): ByteArray = _appPublicKey.clone()
}
