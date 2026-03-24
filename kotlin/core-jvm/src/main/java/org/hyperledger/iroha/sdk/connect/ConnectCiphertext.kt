package org.hyperledger.iroha.sdk.connect

/** Decoded ciphertext payload from a Connect frame. */
class ConnectCiphertext internal constructor(
    @JvmField val direction: ConnectDirection,
    aead: ByteArray,
) {
    private val _aead: ByteArray = aead.copyOf()

    fun aead(): ByteArray = _aead.clone()
}
